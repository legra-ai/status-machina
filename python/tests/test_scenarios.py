"""Native scenario replay: kernel-decided bounds, traces, assertions and typed rejections."""
from dataclasses import dataclass, field
from enum import Enum, IntEnum, auto
from itertools import count
import unittest
from status_machina import (
    Action, AssertionLimitError, Behavior, BehaviorRef, BehaviorRegistry, BehaviorRole, CallbackError, Condition,
    CompleteWithinScenarios, Counterexample, Definition, Disposition, DispatchFailedStep, Effect,
    EmptyScenarioSetError, Event, ExecutionFailed, IgnoredStep, LimitError, LimitReached, Limits,
    MissingAssertionPredicateError, OutputEvent, RefusedStep, RoleMismatchError, Scenario, ScenarioAssertion,
    ScenarioCountLimitError, ScenarioError, ScenarioFailure, ScenarioLimit, ScenarioLimits, ScenarioLimitsError,
    ScenarioNodeLimitError, ScenarioObservation, ScenarioReplay, ScenarioSet, ScenarioStep, ScenarioStepLimitError,
    SnapshotError, State, Transition, TransitionedStep, UnknownBehaviorError,
)

class Phase(Enum):
    CART = auto()
    PAID = auto()
    SHIPPED = auto()

class Input(Enum):
    PAY = auto()
    SHIP = auto()
    RUSH = auto()
    UNDECLARED = auto()

class Story(Enum):
    NEVER_UNPAID = auto()
    PAY = auto()

class Code(IntEnum):
    ONE = 1

@dataclass
class Order:
    paid: list[int] = field(default_factory=list)

def pay(data, amount, facts):
    data.paid.append(amount)

def shipped_unpaid(observation):
    return not (observation.before is Phase.CART and observation.after is Phase.SHIPPED)

def definition(*, rush=True, invariant=None, behaviors=BehaviorRegistry(), limits=Limits()):
    transitions = [
        Transition(id="pay", source=Phase.CART, event=Input.PAY, target=Phase.PAID,
            guard=Condition(id="positive", predicate=lambda data, amount, facts: amount > 0),
            actions=(Action(id="record", apply=pay),)),
        Transition(id="ship", source=Phase.PAID, event=Input.SHIP, target=Phase.SHIPPED,
            effects=(Effect(id="label", describe=lambda data, payload, facts: "label"),),
            outputs=(OutputEvent(kind="shipped", describe=lambda data, payload, facts: sum(data.paid)),)),
    ]
    if rush:
        # The defect a safety assertion should find: rushing skips payment.
        transitions.append(Transition(id="rush", source=Phase.CART, event=Input.RUSH, target=Phase.SHIPPED))
    return Definition(id="orders", initial=Phase.CART, states=(State(Phase.CART), State(Phase.PAID),
        State(Phase.SHIPPED, terminal=True, invariant=invariant)), transitions=tuple(transitions),
        dispositions=(Disposition(id="repeat", source=Phase.PAID, event=Input.PAY,
            condition=Condition(id="any", predicate=lambda data, amount, facts: True), reason="already paid"),),
        behaviors=behaviors, limits=limits)

def step(kind, payload=None, facts=None):
    return ScenarioStep(Event(kind, payload), facts)

def scenarios(*scenarios, limits=ScenarioLimits()):
    return ScenarioSet(scenarios=tuple(Scenario(identity, tuple(steps)) for identity, steps in scenarios), limits=limits)

class ScenarioReplayTests(unittest.TestCase):
    def test_complete_replay_observes_native_identities_and_counts_coverage(self):
        seen: list[ScenarioObservation] = []
        def record(observation):
            seen.append(observation)
            return True
        set_ = scenarios(("happy", [step(Input.PAY, 10), step(Input.PAY, 5), step(Input.SHIP)]),
            ("stray", [step(Input.UNDECLARED)]))
        report = ScenarioReplay.run(definition(), id="replay", data=Order(), scenarios=set_,
            assertions=(ScenarioAssertion("record", record),))
        self.assertEqual(report.result, CompleteWithinScenarios(scenarios=2, nodes=4))
        self.assertEqual([(o.scenario, o.step, o.input_event, o.before, o.after) for o in seen], [
            ("happy", 0, Input.PAY, Phase.CART, Phase.PAID), ("happy", 1, Input.PAY, Phase.PAID, Phase.PAID),
            ("happy", 2, Input.SHIP, Phase.PAID, Phase.SHIPPED), ("stray", 0, Input.UNDECLARED, Phase.CART, Phase.CART)])
        self.assertEqual([o.outcome for o in seen], [TransitionedStep("pay"), IgnoredStep("repeat"),
            TransitionedStep("ship"), RefusedStep()])
        self.assertEqual((seen[2].output_events, seen[2].effects), (("shipped",), ("label",)))

    def test_counterexample_is_the_bounded_prefix_through_the_failing_step(self):
        later = []
        report = ScenarioReplay.run(definition(), id="replay", data=Order(), scenarios=scenarios(
            ("paid", [step(Input.PAY, 1), step(Input.SHIP)]), ("rushed", [step(Input.UNDECLARED), step(Input.RUSH)]),
            ("never", [step(Input.PAY, 1)])), assertions=(ScenarioAssertion("never-unpaid", shipped_unpaid),
            ScenarioAssertion("later", lambda o: later.append(o.step) or True)))
        self.assertIsInstance(report.result, Counterexample)
        assert isinstance(report.result, Counterexample)
        trace = report.result.trace
        self.assertEqual((trace.scenario, trace.assertion), ("rushed", "never-unpaid"))
        self.assertEqual([(o.step, o.outcome) for o in trace.steps], [(0, RefusedStep()), (1, TransitionedStep("rush"))])
        # Assertions run in order and stop at the first failure; the later scenario never runs.
        self.assertEqual(later, [0, 1, 0])

    def test_every_scenario_starts_from_an_isolated_snapshot(self):
        original = Order([7])
        totals = []
        registry = BehaviorRegistry((Behavior(BehaviorRef(Story.NEVER_UNPAID, "v1"), BehaviorRole.ASSERTION,
            shipped_unpaid),))
        report = ScenarioReplay.run(definition(behaviors=registry, rush=False), id="replay", data=original,
            scenarios=scenarios(("a", [step(Input.PAY, 3), step(Input.SHIP)]), ("b", [step(Input.PAY, 4)])),
            assertions=(ScenarioAssertion(Story.NEVER_UNPAID, BehaviorRef(Story.NEVER_UNPAID, "v1")),
                ScenarioAssertion("totals", lambda o: totals.append(o.output_events) or True)))
        self.assertEqual(report.result, CompleteWithinScenarios(scenarios=2, nodes=3))
        self.assertEqual(original.paid, [7])
        self.assertEqual(totals, [(), ("shipped",), ()])

    def test_dynamic_budgets_stop_replay_before_the_unchecked_step(self):
        set_ = scenarios(("a", [step(Input.PAY, 1), step(Input.SHIP)]), limits=ScenarioLimits(max_outputs=1))
        report = ScenarioReplay.run(definition(), id="replay", data=Order(), scenarios=set_)
        self.assertEqual(report.result, LimitReached("a", 1, ScenarioLimit.OUTPUTS))
        set_ = scenarios(("a", [step(Input.PAY, 1), step(Input.PAY, 1)]), limits=ScenarioLimits(max_trace_entries=1))
        report = ScenarioReplay.run(definition(), id="replay", data=Order(), scenarios=set_)
        self.assertEqual(report.result, LimitReached("a", 1, ScenarioLimit.TRACE_ENTRIES))

    def test_rejected_dispatch_and_invalid_initial_data_are_execution_failures(self):
        observed = []
        report = ScenarioReplay.run(definition(invariant=lambda data: False), id="replay", data=Order(),
            scenarios=scenarios(("a", [step(Input.PAY, 1), step(Input.SHIP)])),
            assertions=(ScenarioAssertion("watch", lambda o: observed.append(o.outcome) or True),))
        self.assertIsInstance(report.result, ExecutionFailed)
        assert isinstance(report.result, ExecutionFailed)
        self.assertEqual((report.result.scenario, report.result.step, report.result.failure),
            ("a", 1, ScenarioFailure.INVARIANT_FAILED))
        self.assertEqual(report.result.steps[-1].outcome, DispatchFailedStep())
        self.assertEqual(report.result.steps[-1].after, Phase.PAID)
        self.assertEqual(observed[-1], DispatchFailedStep())
        # Replay data that fails the initial state's invariant fails at step 0, with no steps.
        guarded = Definition(id="d", initial=Phase.CART, states=(State(Phase.CART,
            invariant=lambda data: not data.paid),), transitions=())
        report = ScenarioReplay.run(guarded, id="replay", data=Order([1]), scenarios=scenarios(("a", [step(Input.PAY)])))
        self.assertEqual(report.result, ExecutionFailed("a", 0, ScenarioFailure.INVALID_INITIAL_DATA, ()))

    def test_scenarios_are_positions_and_shared_inputs_fit_a_small_arena(self):
        facts = object()
        set_ = ScenarioSet(scenarios=(Scenario(1, (step(Input.UNDECLARED, None, facts),) * 200),
            Scenario(True, (step(Input.RUSH, None, facts),)), Scenario(Code.ONE, (step(Input.RUSH),))))
        report = ScenarioReplay.run(definition(limits=Limits(max_values=16)), id="replay", data=Order(),
            scenarios=set_, assertions=(ScenarioAssertion("never-unpaid", shipped_unpaid),))
        assert isinstance(report.result, Counterexample)
        self.assertIs(type(report.result.trace.scenario), bool)

class ScenarioRejectionTests(unittest.TestCase):
    def test_limits_are_checked_by_the_kernel(self):
        self.assertEqual(ScenarioLimits(), ScenarioLimits(max_scenarios=64, max_steps_per_scenario=256,
            max_nodes=4096, max_outputs=4096, max_trace_entries=4096))
        for invalid in ({"max_scenarios": 0}, {"max_nodes": 65537}, {"max_trace_entries": 2**80},
                {"max_outputs": -1}):
            with self.assertRaises(ScenarioLimitsError) as caught:
                ScenarioLimits(**invalid)
            self.assertIsInstance(caught.exception, LimitError)
            self.assertEqual(caught.exception.context["reason"], "outside_supported_range")
            self.assertEqual(caught.exception.context[next(iter(invalid))], next(iter(invalid.values())))
        for wrong in ({"max_scenarios": True}, {"max_nodes": 1.0}):
            with self.assertRaises(TypeError):
                ScenarioLimits(**wrong)

    def test_sets_are_bounded_lazily_by_the_kernel(self):
        with self.assertRaises(EmptyScenarioSetError):
            ScenarioSet(scenarios=())
        with self.assertRaises(ScenarioCountLimitError) as caught:
            ScenarioSet(scenarios=(Scenario(n, ()) for n in count()), limits=ScenarioLimits(max_scenarios=2))
        self.assertEqual((caught.exception.context["limit"], caught.exception.context["scenario"]), (2, 2))
        endless = (step(Input.PAY) for _ in count())
        with self.assertRaises(ScenarioStepLimitError) as caught:
            ScenarioSet(scenarios=(Scenario("a", endless),))
        self.assertEqual(caught.exception.context["limit"], 256)
        with self.assertRaises(ScenarioNodeLimitError) as node:
            scenarios(("a", [step(Input.PAY)] * 2), ("b", [step(Input.PAY)] * 2), limits=ScenarioLimits(max_nodes=3))
        self.assertEqual((node.exception.context["limit"], node.exception.context["scenario"]), (3, "b"))
        self.assertIsInstance(node.exception, ScenarioError)
        with self.assertRaises(TypeError):
            ScenarioSet(scenarios=(("a", ()),))  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            Scenario("a", (Event(Input.PAY, None),))  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            ScenarioStep((Input.PAY, None), None)  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            Scenario([], ())

    def test_assertions_need_a_synchronous_predicate_or_exact_reference(self):
        with self.assertRaises(MissingAssertionPredicateError) as caught:
            ScenarioAssertion("a", None)  # type: ignore[arg-type]
        self.assertEqual((caught.exception.context["reason"], caught.exception.context["assertion"]),
            ("missing_predicate", "a"))
        async def asynchronous(observation):
            return True
        for invalid in (True, asynchronous):
            with self.assertRaises(TypeError):
                ScenarioAssertion("a", invalid)  # type: ignore[arg-type]

    def test_assertion_references_resolve_exactly_or_fail_typed(self):
        registry = BehaviorRegistry((Behavior(BehaviorRef(Story.PAY, "v1"), BehaviorRole.ACTION, pay),))
        set_ = scenarios(("a", [step(Input.PAY, 1)]))
        machine = definition(behaviors=registry)
        with self.assertRaises(UnknownBehaviorError) as unknown:
            ScenarioReplay.run(machine, id="r", data=Order(), scenarios=set_, assertions=(
                ScenarioAssertion("fine", shipped_unpaid),
                ScenarioAssertion("missing", BehaviorRef(Story.NEVER_UNPAID, "v1"))))
        self.assertEqual({k: unknown.exception.context[k] for k in ("story", "implementation", "entry", "assertion")},
            {"story": Story.NEVER_UNPAID, "implementation": "v1", "entry": 1, "assertion": "missing"})
        with self.assertRaises(RoleMismatchError) as mismatch:
            ScenarioReplay.run(machine, id="r", data=Order(), scenarios=set_,
                assertions=(ScenarioAssertion("pay", BehaviorRef(Story.PAY, "v1")),))
        self.assertEqual((mismatch.exception.context["expected"], mismatch.exception.context["actual"]),
            (BehaviorRole.ASSERTION, BehaviorRole.ACTION))
        with self.assertRaises(AssertionLimitError) as limit:
            ScenarioReplay.run(machine, id="r", data=Order(), scenarios=set_,
                assertions=(ScenarioAssertion(n, shipped_unpaid) for n in count()))
        self.assertEqual(limit.exception.context["limit"], 128)
        with self.assertRaises(TypeError):
            ScenarioReplay.run(machine, id="r", data=Order(), scenarios=set_, assertions=(shipped_unpaid,))  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            ScenarioReplay.run(object(), id="r", data=Order(), scenarios=set_)  # type: ignore[arg-type]

    def test_callback_failures_stop_replay_with_their_original_cause(self):
        set_ = scenarios(("a", [step(Input.PAY, 1)]))
        failure = ValueError("assertion raised")
        def raising(observation):
            raise failure
        with self.assertRaises(CallbackError) as caught:
            ScenarioReplay.run(definition(), id="r", data=Order(), scenarios=set_,
                assertions=(ScenarioAssertion("raising", raising),))
        self.assertIs(caught.exception.__cause__, failure)
        self.assertEqual(caught.exception.phase, "assertion")
        with self.assertRaises(CallbackError) as caught:
            ScenarioReplay.run(definition(), id="r", data=Order(), scenarios=set_,
                assertions=(ScenarioAssertion("truthy", lambda o: 1),))
        self.assertIsInstance(caught.exception.__cause__, TypeError)
        guard = ValueError("guard raised")
        def bad_guard(data, amount, facts):
            raise guard
        broken = Definition(id="d", initial=Phase.CART, states=(State(Phase.CART), State(Phase.PAID)),
            transitions=(Transition(id="t", source=Phase.CART, event=Input.PAY, target=Phase.PAID,
                guard=Condition(id="g", predicate=bad_guard)),))
        with self.assertRaises(CallbackError) as caught:
            ScenarioReplay.run(broken, id="r", data=Order(), scenarios=set_)
        self.assertIs(caught.exception.__cause__, guard)
        snapshot = ValueError("snapshot raised")
        def bad_snapshot(value):
            raise snapshot
        copying = Definition(id="d", initial=Phase.CART, states=(State(Phase.CART),), transitions=(),
            snapshot=bad_snapshot)
        with self.assertRaises(SnapshotError) as snap:
            ScenarioReplay.run(copying, id="r", data=Order(), scenarios=set_)
        self.assertIs(snap.exception.__cause__, snapshot)

if __name__ == "__main__":
    unittest.main()
