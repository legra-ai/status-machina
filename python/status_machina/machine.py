"""Checked definitions, owned instances and single-use decisions."""
from __future__ import annotations
import json
import inspect
from typing import Any, Callable, Generic, Hashable, Iterable, NoReturn, TypeAlias, cast
from typing_extensions import TypeVar
from . import _native
from ._compile import Compiler
from ._bounded import bounded
from ._context import Context, DEFAULT_SNAPSHOT, translate
from .errors import DefinitionError, DisposedError
from .model import (BehaviorRegistry, Disposition, EffectIntent, Event, Ignored, Limits,
    Capability, CapabilityReport, CapabilityStatus, OutputIntent, Refused, State, Transition, Transitioned, TransitionRecord)

StateT = TypeVar("StateT", bound=Hashable)
KindT = TypeVar("KindT", bound=Hashable)
DataT = TypeVar("DataT", default=Any)
Outcome: TypeAlias = Refused[StateT, KindT] | Ignored[StateT, KindT] | Transitioned[StateT]

class Definition(Generic[StateT, KindT, DataT]):
    """Immutable checked graph. Identifiers remain caller-defined native values."""
    def __init__(self, *, id: Hashable, initial: StateT,
            states: Iterable[State[StateT, DataT]], transitions: Iterable[Transition[StateT, KindT, DataT]],
            capabilities: Iterable[Capability[StateT, DataT]] = (),
            dispositions: Iterable[Disposition] = (), behaviors: BehaviorRegistry = BehaviorRegistry(),
            snapshot: Callable[[DataT], DataT] = DEFAULT_SNAPSHOT, input_snapshot: Callable[[Any], Any] = DEFAULT_SNAPSHOT, limits: Limits = Limits()) -> None:
        for function in (snapshot, input_snapshot):
            if not callable(function) or inspect.iscoroutinefunction(function) or inspect.iscoroutinefunction(getattr(function, "__call__", None)):
                raise DefinitionError("Snapshot functions must be synchronous callables")
        self._id = id
        self._initial = initial
        self._limits = limits
        self._states = bounded(states, limits.max_states, "state inventory")
        self._transitions = bounded(transitions, limits.max_transitions, "transition inventory")
        self._dispositions = bounded(dispositions, 128, "disposition inventory")
        self._capabilities = bounded(capabilities, 4096, "capability inventory")
        self._behaviors = behaviors
        self._snapshot = snapshot
        self._input_snapshot = input_snapshot
        compiler = Compiler(limits)
        compiler.ids.intern("definition", id)
        source = {"initial": compiler.ids.intern("state", initial),
            "max_states": limits.max_states, "max_transitions": limits.max_transitions,
            "max_bindings": behaviors.max_bindings,
            "states": [compiler.state(state) for state in self._states],
            "transitions": [compiler.transition(edge) for edge in self._transitions],
            "dispositions": [compiler.disposition(rule) for rule in self._dispositions],
            "behaviors": compiler.behaviors(behaviors),
            "capabilities": [{"id": compiler.ids.intern("capability", value.id),
                "implementation": compiler.implementation(value.evaluate)} for value in self._capabilities]}
        self._compiler = compiler
        self._callbacks = tuple(compiler.callbacks)
        try:
            self._native = _native._Definition(json.dumps(source, separators=(",", ":")))
        except _native.NativeFailure as error:
            translate(error, compiler.ids)
        if not callable(snapshot) or not callable(input_snapshot):
            raise DefinitionError("snapshot and input_snapshot must be callable")

    @property
    def id(self) -> Hashable:
        return self._id

    @property
    def initial(self) -> StateT:
        return self._initial

    @property
    def states(self) -> tuple[State[StateT, DataT], ...]:
        return self._states

    @property
    def transitions(self) -> tuple[Transition[StateT, KindT, DataT], ...]:
        return self._transitions

    @property
    def capabilities(self) -> tuple[Capability[StateT, DataT], ...]:
        return self._capabilities

    def instantiate(self, *, id: Hashable, data: DataT) -> Machine[StateT, KindT, DataT]:
        hash((type(id), id))
        context = Context(self, id)
        try:
            native = self._native.instantiate(context.allocate(data), self._limits.max_values, context.invoke)
            context.native = native
            if context.arena_id != native.arena_id():
                raise RuntimeError("Native arena identity changed during admission")
        except _native.NativeFailure as error:
            context.values.clear()
            translate(error, self._compiler.ids)
        context.sweep()
        return Machine(context)

class Machine(Generic[StateT, KindT, DataT]):
    """An owned instance. Callbacks are synchronous; external effects stay caller-owned."""
    def __init__(self, context):
        self._context = context

    @property
    def id(self) -> Hashable:
        if self._context.closed:
            raise DisposedError("The machine has been disposed")
        return self._context.identity

    @property
    def state(self) -> StateT:
        with self._context.operation() as context:
            return context.ids.resolve("state", context.native.state())

    @property
    def revision(self) -> int:
        with self._context.operation() as context:
            return context.native.revision()

    @property
    def data(self) -> DataT:
        with self._context.operation() as context:
            return context.resolve(json.loads(context.native.snapshot(context.invoke)))

    def _outcome(self, encoded, event=None) -> Outcome[StateT, KindT]:
        value = json.loads(encoded)
        ids = self._context.ids
        if value["kind"] == "refused":
            return Refused(ids.resolve("state", value["state"]), event.kind)
        if value["kind"] == "ignored":
            return Ignored(ids.resolve("disposition", value["disposition"]),
                ids.resolve("state", value["state"]), event.kind, value["reason"])
        if value["kind"] != "transitioned":
            raise RuntimeError("Unrecognized native dispatch outcome")
        record = value["record"]
        return Transitioned(TransitionRecord(ids.resolve("transition", record["transition"]),
            ids.resolve("state", record["from_state"]), ids.resolve("state", record["to_state"]),
            int(record["previous_revision"]), int(record["revision"])),
            tuple(EffectIntent(ids.resolve("effect", item["id"]), self._context.resolve(item["payload"])) for item in value["effects"]),
            tuple(OutputIntent(ids.resolve("event", item["id"]), self._context.resolve(item["payload"])) for item in value["outputs"]))

    def dispatch(self, event: Event[KindT, object], *, facts: object = None, expected_revision: int | None = None) -> Outcome[StateT, KindT]:
        with self._context.operation() as context:
            encoded = context.native.dispatch(context.ids.find("event", event.kind),
                context.allocate(event.payload), context.allocate(facts), context.invoke, expected_revision)
            return self._outcome(encoded, event)

    def evaluate(self, event: Event[KindT, object], *, facts: object = None, expected_revision: int | None = None) -> Decision:
        with self._context.operation() as context:
            decision = context.native.evaluate(context.ids.find("event", event.kind),
                context.allocate(event.payload), context.allocate(facts), context.invoke, expected_revision)
            return Decision(context, decision, event.kind)

    def evaluate_capability(self, capability: Hashable, *, facts: object) -> CapabilityReport:
        """Observe an advisory policy at this instance's exact current revision."""
        with self._context.operation() as context:
            raw = json.loads(context.native.evaluate_capability(context.ids.find("capability", capability),
                context.allocate(facts), context.invoke))
            return CapabilityReport(context.ids.resolve("capability", raw["capability"]),
                context.identity, int(raw["revision"]), CapabilityStatus(raw["status"]))

    def accept(self, decision: Decision) -> Outcome[StateT, KindT]:
        if decision._context is None:
            raise DisposedError("The decision has already been consumed")
        with self._context.operation() as context:
            source = decision._context
            decision._context = None
            try:
                return self._outcome(context.native.accept(decision._native), Event(decision._event_kind, None))
            finally:
                if source is not context:
                    source.sweep()

    def dispose(self) -> None:
        with self._context.operation(allow_closed=True) as context:
            if not context.closed:
                context.native.dispose()
                context.detach()

    close = dispose

    def __enter__(self) -> Machine[StateT, KindT, DataT]:
        if self._context.closed:
            raise DisposedError("The machine has been disposed")
        return self

    def __exit__(self, *_):
        self.dispose()

class Decision:
    """An unaccepted candidate; acceptance consumes it exactly once."""
    def __init__(self, context, native, event_kind):
        self._context = context
        self._native = native
        self._event_kind = event_kind

    @property
    def expected_revision(self) -> int:
        if self._context is None:
            raise DisposedError("The decision has already been consumed")
        try:
            return self._native.expected_revision()
        except _native.NativeFailure as error:
            translate(error, self._context.ids)

    def dispose(self) -> None:
        if self._context is None:
            return
        context = self._context
        with context.operation(allow_closed=True):
            self._native.dispose()
            self._context = None

    close = dispose
    def __enter__(self):
        _ = self.expected_revision
        return self
    def __exit__(self, *_):
        self.dispose()
