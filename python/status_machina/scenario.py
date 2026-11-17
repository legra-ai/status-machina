"""Bounded replay of finite caller-supplied scenarios, decided by the kernel's ``ScenarioReplay``.

A scenario is an ordered list of event/fact steps. Replay runs each scenario on a
fresh instance of one definition, from a snapshot of the same initial data, and
checks every independent safety assertion after every step. The report is
explicitly bounded to the supplied scenarios: ``CompleteWithinScenarios`` is not
a proof over all inputs. Traces hold identities only, never payloads or data.

The kernel decides every limit, set bound, assertion order, dispatch, budget and
trace rule; this module interns native identities and resolves the report back
to them. Replay schedules nothing and never dispatches output events.
"""
from __future__ import annotations

import builtins
import inspect
import json
from collections.abc import Callable, Hashable, Iterable
from dataclasses import dataclass, field
from enum import Enum
from itertools import islice
from typing import Any, Generic, Literal, NoReturn, TypeAlias

from typing_extensions import TypeVar

from . import _native, errors
from ._context import Context, is_asynchronous, translate
from .machine import Definition
from .model import BehaviorRef, BehaviorRole, Event

ScenarioT = TypeVar("ScenarioT", bound=Hashable, default=Hashable)
AssertionT = TypeVar("AssertionT", bound=Hashable, default=Hashable)
StateT = TypeVar("StateT", bound=Hashable, default=Hashable)
KindT = TypeVar("KindT", bound=Hashable, default=Hashable)
DataT = TypeVar("DataT", default=Any)
PayloadT = TypeVar("PayloadT", default=Any)
FactsT = TypeVar("FactsT", default=Any)

# Kernel ceilings: they bound how many native entries are read, never what is accepted.
HARD_MAX_SCENARIOS = 1024
HARD_MAX_STEPS_PER_SCENARIO = 4096
HARD_MAX_ASSERTIONS = 128
_LIMITS = ("max_scenarios", "max_steps_per_scenario", "max_nodes", "max_outputs", "max_trace_entries")
# The kernel's conservative defaults, resolved by its own builder.
_DEFAULTS: dict[str, int] = json.loads(_native.scenario_limits("{}"))
_FAILURES: dict[tuple[str, str], type[errors.StatusMachinaError]] = {
    ("limits", "outside_supported_range"): errors.ScenarioLimitsError,
    ("set", "empty"): errors.EmptyScenarioSetError,
    ("set", "scenario_limit_exceeded"): errors.ScenarioCountLimitError,
    ("set", "step_limit_exceeded"): errors.ScenarioStepLimitError,
    ("set", "total_node_limit_exceeded"): errors.ScenarioNodeLimitError,
    ("assertion", "missing_predicate"): errors.MissingAssertionPredicateError,
    ("assertion", "unknown_behavior"): errors.UnknownBehaviorError,
    ("assertion", "unknown_implementation"): errors.UnknownImplementationError,
    ("assertion", "role_mismatch"): errors.RoleMismatchError,
    ("replay", "assertion_limit_exceeded"): errors.AssertionLimitError,
}

def _raise(error: Exception, **context: object) -> NoReturn:
    info = json.loads(str(error))
    exception = _FAILURES.get((info.get("family"), info.get("reason")))
    if info.get("code") != "scenario" or exception is None:
        raise RuntimeError("Unrecognized native scenario failure") from error
    context["reason"] = info["reason"]
    for name in ("limit", "entry"):
        if info[name] is not None:
            context[name] = info[name]
    for name in ("expected", "actual"):
        if info[f"{name}_role"] is not None:
            context[name] = BehaviorRole(info[f"{name}_role"])
    raise exception(info["message"], **context) from None

def _bound(value: object) -> int:
    """A bound the kernel can receive; anything outside ``u64`` is outside every ceiling too."""
    return value if type(value) is int and 0 <= value < 2**64 else (0 if type(value) is int and value < 0 else 2**64 - 1)

@dataclass(frozen=True, kw_only=True)
class ScenarioLimits:
    """Positive replay budgets, validated by the kernel; defaults are the kernel's own."""
    max_scenarios: int = _DEFAULTS["max_scenarios"]
    max_steps_per_scenario: int = _DEFAULTS["max_steps_per_scenario"]
    max_nodes: int = _DEFAULTS["max_nodes"]
    max_outputs: int = _DEFAULTS["max_outputs"]
    max_trace_entries: int = _DEFAULTS["max_trace_entries"]

    def __post_init__(self) -> None:
        values = {name: getattr(self, name) for name in _LIMITS}
        for name, value in values.items():
            if type(value) is not int:
                raise TypeError(f"{name} must be an integer")
        try:
            _native.scenario_limits(json.dumps({name: _bound(value) for name, value in values.items()}))
        except _native.NativeFailure as error:
            _raise(error, **values)

    def _encode(self) -> dict[str, int]:
        return {name: getattr(self, name) for name in _LIMITS}

@dataclass(frozen=True)
class ScenarioStep(Generic[KindT, PayloadT, FactsT]):
    """One dispatch: the event and the facts it is evaluated against."""
    event: Event[KindT, PayloadT]
    facts: FactsT

    def __post_init__(self) -> None:
        if not isinstance(self.event, Event):
            raise TypeError("A ScenarioStep needs an Event")

@dataclass(frozen=True)
class Scenario(Generic[ScenarioT, KindT]):
    """An identified, ordered, finite list of steps; at most 4097 steps are read."""
    id: ScenarioT
    steps: tuple[ScenarioStep[KindT, Any, Any], ...]

    def __post_init__(self) -> None:
        hash((type(self.id), self.id))
        steps = tuple(islice(self.steps, HARD_MAX_STEPS_PER_SCENARIO + 1))
        for step in steps:
            if not isinstance(step, ScenarioStep):
                raise TypeError("Scenario steps must be ScenarioStep values")
        object.__setattr__(self, "steps", steps)

@dataclass(frozen=True, kw_only=True)
class ScenarioSet(Generic[ScenarioT, KindT]):
    """A non-empty bounded set of scenarios, checked by the kernel's ``ScenarioSet`` builder."""
    scenarios: tuple[Scenario[ScenarioT, KindT], ...]
    limits: ScenarioLimits = field(default_factory=ScenarioLimits)

    def __post_init__(self) -> None:
        if not isinstance(self.limits, ScenarioLimits):
            raise TypeError("limits must be ScenarioLimits")
        scenarios = tuple(islice(self.scenarios, min(self.limits.max_scenarios, HARD_MAX_SCENARIOS) + 1))
        for scenario in scenarios:
            if not isinstance(scenario, Scenario):
                raise TypeError("A ScenarioSet holds Scenario values")
        object.__setattr__(self, "scenarios", scenarios)
        source = {"limits": self.limits._encode(), "steps": [len(scenario.steps) for scenario in scenarios]}
        try:
            _native.scenario_set(json.dumps(source))
        except _native.NativeFailure as error:
            entry = json.loads(str(error))["entry"]
            context = {} if entry is None else {"scenario": scenarios[entry].id}
            _raise(error, **context)

@dataclass(frozen=True)
class TransitionedStep:
    """One transition was accepted."""
    transition: Hashable
    kind: Literal["transitioned"] = field(default="transitioned", init=False)

@dataclass(frozen=True)
class IgnoredStep:
    """A declared disposition accepted the event without a state change."""
    disposition: Hashable
    kind: Literal["ignored"] = field(default="ignored", init=False)

@dataclass(frozen=True)
class RefusedStep:
    """No transition or disposition accepted the event."""
    kind: Literal["refused"] = field(default="refused", init=False)

@dataclass(frozen=True)
class DispatchFailedStep:
    """Dispatch was rejected; no callback error text is retained."""
    kind: Literal["dispatch_failed"] = field(default="dispatch_failed", init=False)

ScenarioStepKind: TypeAlias = TransitionedStep | IgnoredStep | RefusedStep | DispatchFailedStep

@dataclass(frozen=True)
class ScenarioObservation(Generic[ScenarioT, StateT, KindT]):
    """Compact metadata of one replayed step; Rust ``kind()`` is ``outcome``. No payloads."""
    scenario: ScenarioT
    step: int
    input_event: KindT
    before: StateT
    after: StateT
    outcome: ScenarioStepKind
    output_events: tuple[Hashable, ...]
    effects: tuple[Hashable, ...]

@dataclass(frozen=True)
class ScenarioAssertion(Generic[AssertionT]):
    """One independently named safety predicate, true while the property holds.

    ``evaluate`` is a synchronous callable or an exact ``BehaviorRef`` to an
    ``ASSERTION`` implementation in the definition's registry. It must be pure,
    total and bounded; the binding cannot prove that of application code.
    """
    id: AssertionT
    evaluate: Callable[[ScenarioObservation[Any, Any, Any]], bool] | BehaviorRef

    def __post_init__(self) -> None:
        hash((type(self.id), self.id))
        predicate: object = self.evaluate
        if predicate is not None and not isinstance(predicate, BehaviorRef):
            if not callable(predicate):
                raise TypeError("evaluate must be a callable or an exact BehaviorRef")
            if is_asynchronous(predicate):
                raise TypeError("Scenario assertions must be synchronous")
        try:
            _native.scenario_assertion(predicate is not None)
        except _native.NativeFailure as error:
            _raise(error, assertion=self.id)

class ScenarioFailure(Enum):
    """Dispatch error category of a failed step; no callback error text is retained."""
    EVALUATION_FAILED = "evaluation_failed"
    INVALID_INITIAL_DATA = "invalid_initial_data"
    AMBIGUOUS = "ambiguous"
    INVARIANT_FAILED = "invariant_failed"
    POSTCONDITION_FAILED = "postcondition_failed"
    ACTION_FAILED = "action_failed"
    REVISION_EXHAUSTED = "revision_exhausted"
    DEFINITION_MISMATCH = "definition_mismatch"
    INSTANCE_MISMATCH = "instance_mismatch"
    STALE_REVISION = "stale_revision"

class ScenarioLimit(Enum):
    """The dynamic budget that stopped replay."""
    OUTPUTS = "outputs"
    TRACE_ENTRIES = "trace_entries"

@dataclass(frozen=True)
class CounterexampleTrace(Generic[ScenarioT, StateT, KindT]):
    """The step prefix, through the failing step, that falsified ``assertion``."""
    scenario: ScenarioT
    assertion: Hashable
    steps: tuple[ScenarioObservation[ScenarioT, StateT, KindT], ...]

@dataclass(frozen=True)
class CompleteWithinScenarios:
    """Every supplied step was checked and passed: coverage of exactly these scenarios and nodes."""
    scenarios: int
    nodes: int
    kind: Literal["complete_within_scenarios"] = field(default="complete_within_scenarios", init=False)

@dataclass(frozen=True)
class Counterexample(Generic[ScenarioT, StateT, KindT]):
    """An independent assertion failed on this bounded trace."""
    trace: CounterexampleTrace[ScenarioT, StateT, KindT]
    kind: Literal["counterexample"] = field(default="counterexample", init=False)

@dataclass(frozen=True)
class LimitReached(Generic[ScenarioT]):
    """Replay stopped before checking ``step`` because a dynamic budget was reached."""
    scenario: ScenarioT
    step: int
    limit: ScenarioLimit
    kind: Literal["limit_reached"] = field(default="limit_reached", init=False)

@dataclass(frozen=True)
class ExecutionFailed(Generic[ScenarioT, StateT, KindT]):
    """Instantiation (step 0, no steps) or dispatch was rejected; ``steps`` ends at the failed step."""
    scenario: ScenarioT
    step: int
    failure: ScenarioFailure
    steps: tuple[ScenarioObservation[ScenarioT, StateT, KindT], ...]
    kind: Literal["execution_failed"] = field(default="execution_failed", init=False)

ScenarioResult: TypeAlias = (CompleteWithinScenarios | Counterexample[ScenarioT, StateT, KindT]
    | LimitReached[ScenarioT] | ExecutionFailed[ScenarioT, StateT, KindT])

@dataclass(frozen=True)
class BoundedScenarioReport(Generic[ScenarioT, StateT, KindT]):
    """A replay result whose name states its bound: only the supplied scenarios were checked."""
    result: ScenarioResult[ScenarioT, StateT, KindT]

class _Replay(Context):
    """One replay arena: every input is leased up front; candidates are swept when it fills."""
    def __init__(self, definition: Definition[Any, Any, Any], identity: Hashable,
            scenarios: tuple[Scenario[Any, Any], ...], assertions: tuple[ScenarioAssertion[Any], ...]) -> None:
        super().__init__(definition, identity)
        self.scenarios = scenarios
        self.assertions = assertions
        self.running = False

    def allocate(self, value):
        # Only leased keys survive a sweep, and every earlier key is leased once replay runs.
        if self.running and len(self.values) == self.capacity:
            self.sweep()
        return super().allocate(value)

    def observation(self, raw: dict) -> ScenarioObservation:
        ids = self.ids
        scenario = self.scenarios[raw["scenario"]]
        if raw["kind"] == "transitioned":
            outcome: ScenarioStepKind = TransitionedStep(ids.resolve("transition", raw["transition"]))
        elif raw["kind"] == "ignored":
            outcome = IgnoredStep(ids.resolve("disposition", raw["disposition"]))
        elif raw["kind"] == "refused":
            outcome = RefusedStep()
        else:
            outcome = DispatchFailedStep()
        return ScenarioObservation(scenario.id, raw["step"], scenario.steps[raw["step"]].event.kind,
            ids.resolve("state", raw["before"]), ids.resolve("state", raw["after"]), outcome,
            tuple(ids.resolve("event", value) for value in raw["outputs"]),
            tuple(ids.resolve("effect", value) for value in raw["effects"]))

    def lower(self, evaluate: object, position: int) -> dict | None:
        """Registered assertions keep their definition callback; inline ones follow it."""
        if evaluate is None:
            return None
        if isinstance(evaluate, BehaviorRef):
            # An unknown Story or implementation reaches the kernel as an absent index.
            return {"callback": None, "reference": {"story": self.ids.find("story", evaluate.story),
                "implementation": self.ids.find("implementation", evaluate.implementation)}}
        return {"callback": len(self.definition._callbacks) + position, "reference": None}

    def invoke(self, encoded):
        request = json.loads(encoded)
        if request["kind"] != "assertion":
            return super().invoke(encoded)
        registered = self.definition._callbacks
        index = request["callback"]
        if index < len(registered):
            function = registered[index].function
        else:
            function = self.assertions[index - len(registered)].evaluate
        result = function(self.observation(request["observation"]))
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("Scenario assertions must return synchronously, not an awaitable")
        if type(result) is not bool:
            raise TypeError("Scenario assertions must return bool")
        return json.dumps({"kind": "bool", "value": result})

    def result(self, raw: dict) -> ScenarioResult[Any, Any, Any]:
        def steps(values: list) -> tuple[ScenarioObservation, ...]:
            return tuple(self.observation(value) for value in values)
        if raw["kind"] == "complete_within_scenarios":
            return CompleteWithinScenarios(raw["scenarios"], raw["nodes"])
        scenario = self.scenarios[raw["scenario"]].id
        if raw["kind"] == "counterexample":
            return Counterexample(CounterexampleTrace(scenario, self.assertions[raw["assertion"]].id,
                steps(raw["steps"])))
        if raw["kind"] == "limit_reached":
            return LimitReached(scenario, raw["step"], ScenarioLimit(raw["limit"]))
        if raw["kind"] == "execution_failed":
            return ExecutionFailed(scenario, raw["step"], ScenarioFailure(raw["failure"]), steps(raw["steps"]))
        raise RuntimeError("Unrecognized native scenario result")

class ScenarioReplay:
    """Caller-driven replay of explicit finite scenarios on fresh ephemeral instances."""

    @staticmethod
    def run(definition: Definition[StateT, KindT, DataT], *, id: Hashable, data: DataT,
            scenarios: ScenarioSet[ScenarioT, KindT], assertions: Iterable[ScenarioAssertion[Any]] = ()
            ) -> BoundedScenarioReport[ScenarioT, StateT, KindT]:
        """Replay every scenario from a snapshot of ``data``, checking each assertion in order.

        The first failing assertion of a step ends replay with a ``Counterexample``.
        A dispatch rejection ends it with ``ExecutionFailed``, a reached output or
        trace budget with ``LimitReached``. A raised callback, snapshot or assertion
        stops replay with a ``CallbackError`` whose ``__cause__`` is the original
        exception. Each distinct input object occupies one value slot of the
        definition's ``max_values`` for the whole replay.
        """
        if not isinstance(definition, Definition):
            raise TypeError("ScenarioReplay.run needs a Definition")
        if not isinstance(scenarios, ScenarioSet):
            raise TypeError("scenarios must be a ScenarioSet")
        hash((type(id), id))
        checked = tuple(islice(assertions, HARD_MAX_ASSERTIONS + 1))
        for assertion in checked:
            if not isinstance(assertion, ScenarioAssertion):
                raise TypeError("assertions must be ScenarioAssertion values")
        context = _Replay(definition, id, scenarios.scenarios, checked)
        arena = _native._Arena(definition._limits.max_values)
        context.native = arena  # type: ignore[assignment]
        context.arena_id = arena.arena_id()
        ids = context.ids
        keys: dict[int, int] = {}
        def lease(value: object) -> int:
            # Repeated input objects share one lease; callbacks only ever see snapshots of them.
            key = keys.get(builtins.id(value))
            if key is None:
                key = keys[builtins.id(value)] = context.allocate(value)
            return key
        try:
            source = {"limits": scenarios.limits._encode(), "initial": context.allocate(data),
                "scenarios": [{"steps": [{"event": ids.find("event", step.event.kind), "payload": lease(step.event.payload),
                    "facts": lease(step.facts)} for step in scenario.steps]} for scenario in scenarios.scenarios],
                "assertions": [{"id": position, "implementation": context.lower(assertion.evaluate, position)}
                    for position, assertion in enumerate(checked)]}
            context.running = True
            try:
                raw = json.loads(definition._native.replay(arena, json.dumps(source, separators=(",", ":")), context.invoke))
            except _native.NativeFailure as error:
                if json.loads(str(error)).get("code") != "scenario":
                    translate(error, ids)
                entry = json.loads(str(error))["entry"]
                extra: dict[str, object] = {}
                if entry is not None:
                    assertion = checked[entry]
                    extra["assertion"] = assertion.id
                    if isinstance(assertion.evaluate, BehaviorRef):
                        extra["story"] = assertion.evaluate.story
                        extra["implementation"] = assertion.evaluate.implementation
                _raise(error, **extra)
            return BoundedScenarioReport(context.result(raw))
        finally:
            context.running = False
            context.values.clear()
            context.detach()
