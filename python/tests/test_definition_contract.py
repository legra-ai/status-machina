"""Definitions, conditions, dispositions and the construction error taxonomy, row by row."""
import unittest
from enum import Enum, auto
from itertools import count

from status_machina import (
    Action,
    AmbiguousTransitionError,
    CallbackError,
    Condition,
    Definition,
    DefinitionError,
    DefinitionLimitError,
    Disposition,
    DuplicateIdentifierError,
    Event,
    Ignored,
    InitialInvariantError,
    InternalTransitionError,
    InvalidConditionError,
    InvalidDispositionError,
    LimitError,
    Limits,
    Refused,
    State,
    StatusMachinaError,
    TerminalTransitionError,
    Transition,
    UnknownStateError,
    UnreachableStateError,
)


class Phase(Enum):
    OPEN = auto()
    REVIEW = auto()
    CLOSED = auto()
    GHOST = auto()

class Input(Enum):
    SUBMIT = auto()
    CLOSE = auto()
    PING = auto()

def edge(id, source, event, target, **extra):
    return Transition(id=id, source=source, event=event, target=target, **extra)

def machine(states=(Phase.OPEN, Phase.CLOSED), transitions=(), **extra):
    return Definition(id="d", initial=Phase.OPEN, states=tuple(State(s) for s in states),
        transitions=tuple(transitions), **extra)

def always(*_):
    return True

class DefinitionContractTests(unittest.TestCase):
    """MachineDefinition, builders, kinds, reachability and limits."""
    def test_checked_definition_is_immutable_and_keeps_native_values(self):
        submit = edge("submit", Phase.OPEN, Input.SUBMIT, Phase.REVIEW)
        definition = Definition(id="d", initial=Phase.OPEN, states=(State(Phase.OPEN), State(Phase.REVIEW),
            State(Phase.CLOSED, terminal=True)), transitions=[submit, edge("close", Phase.REVIEW, Input.CLOSE, Phase.CLOSED)])
        self.assertEqual((definition.id, definition.initial), ("d", Phase.OPEN))
        self.assertIsInstance(definition.states, tuple)
        self.assertIs(definition.transitions[0], submit)
        with self.assertRaises(AttributeError):
            definition.initial = Phase.CLOSED  # type: ignore[misc]
        with self.assertRaises(AttributeError):
            submit.target = Phase.OPEN  # type: ignore[misc]

    def test_internal_transitions_skip_exit_and_entry_while_external_self_transitions_run_them(self):
        log = []
        def act(name):
            return Action(id=name, apply=lambda data, payload, facts: log.append(name))
        definition = Definition(id="d", initial=Phase.OPEN, states=(State(Phase.OPEN, exit_actions=(act("exit"),),
            entry_actions=(act("entry"),)),), transitions=(edge("again", Phase.OPEN, Input.SUBMIT, Phase.OPEN,
                actions=(act("again"),)), edge("touch", Phase.OPEN, Input.PING, Phase.OPEN, internal=True,
                actions=(act("touch"),))))
        with definition.instantiate(id=1, data=None) as running:
            running.dispatch(Event(Input.SUBMIT, None))
            self.assertEqual(log, ["exit", "again", "entry"])
            log.clear()
            running.dispatch(Event(Input.PING, None))
            self.assertEqual(log, ["touch"])
            self.assertEqual(running.revision, 2)

    def test_graph_rules_raise_typed_errors_naming_native_identities(self):
        cases = [
            (lambda: Definition(id="d", initial=Phase.GHOST, states=(State(Phase.OPEN),), transitions=()),
                UnknownStateError, {"reason": "unknown_initial_state", "state": Phase.GHOST}),
            (lambda: machine(transitions=(edge("t", Phase.OPEN, Input.CLOSE, Phase.GHOST),)),
                UnknownStateError, {"reason": "unknown_transition_state", "transition": "t", "state": Phase.GHOST}),
            (lambda: machine(states=(Phase.OPEN,), dispositions=(Disposition("p", Phase.GHOST, Input.PING,
                Condition("c", always), "ignored"),)),
                UnknownStateError, {"reason": "unknown_disposition_state", "disposition": "p", "state": Phase.GHOST}),
            (lambda: machine(states=(Phase.OPEN, Phase.OPEN)),
                DuplicateIdentifierError, {"reason": "duplicate_state", "state": Phase.OPEN}),
            (lambda: machine(transitions=(edge("t", Phase.OPEN, Input.CLOSE, Phase.CLOSED),
                edge("t", Phase.OPEN, Input.SUBMIT, Phase.CLOSED))),
                DuplicateIdentifierError, {"reason": "duplicate_transition", "transition": "t"}),
            (lambda: machine(states=(Phase.OPEN, Phase.CLOSED)),
                UnreachableStateError, {"reason": "unreachable_state", "state": Phase.CLOSED}),
            (lambda: Definition(id="d", initial=Phase.OPEN, states=(State(Phase.OPEN, terminal=True),),
                transitions=(edge("t", Phase.OPEN, Input.CLOSE, Phase.OPEN),)),
                TerminalTransitionError, {"reason": "terminal_has_outgoing_transition", "state": Phase.OPEN}),
            (lambda: machine(transitions=(edge("t", Phase.OPEN, Input.CLOSE, Phase.CLOSED, internal=True),)),
                InternalTransitionError, {"reason": "internal_transition_must_be_self", "transition": "t"}),
        ]
        for build, kind, context in cases:
            with self.subTest(reason=context["reason"]):
                with self.assertRaises(kind) as caught:
                    build()
                self.assertIsInstance(caught.exception, DefinitionError)
                self.assertEqual({key: caught.exception.context[key] for key in context}, context)

    def test_limits_are_kernel_bounds_read_one_entry_beyond(self):
        consumed = []
        def states():
            for index in count():
                consumed.append(index)
                yield State((Phase.OPEN, index))
        with self.assertRaises(DefinitionLimitError) as caught:
            Definition(id="d", initial=(Phase.OPEN, 0), states=states(), transitions=(), limits=Limits(max_states=2))
        self.assertEqual((caught.exception.context["reason"], caught.exception.context["limit"]), ("state_limit_exceeded", 2))
        self.assertIsInstance(caught.exception, LimitError)
        self.assertEqual(len(consumed), 3)
        too_many = tuple(edge(n, Phase.OPEN, Input.CLOSE, Phase.CLOSED) for n in range(17))
        with self.assertRaises(DefinitionLimitError):
            machine(transitions=too_many, limits=Limits(max_transitions=16))
        actions = tuple(Action(id=n, apply=lambda *_: None) for n in range(17))
        with self.assertRaises(DefinitionLimitError) as caught:
            machine(transitions=(edge("t", Phase.OPEN, Input.CLOSE, Phase.CLOSED, actions=actions),))
        self.assertEqual((caught.exception.context["reason"], caught.exception.context["transition"]),
            ("action_limit_exceeded", "t"))
        for invalid in ({"max_states": 0}, {"max_values": 65537}, {"max_transitions": True}):
            with self.assertRaises(LimitError):
                Limits(**invalid)

    def test_initial_data_is_checked_against_the_initial_invariant(self):
        guarded = Definition(id="d", initial=Phase.OPEN, states=(State(Phase.OPEN, invariant=lambda data: data > 0),),
            transitions=())
        with guarded.instantiate(id=1, data=1) as accepted:
            self.assertEqual(accepted.data, 1)
        with self.assertRaises(InitialInvariantError) as caught:
            guarded.instantiate(id=2, data=0)
        self.assertEqual(caught.exception.context["state"], Phase.OPEN)

class ConditionContractTests(unittest.TestCase):
    """Condition expressions: predicate, all/any/not, bounds, short-circuit and exceptions."""
    def evaluate(self, guard, payload=None):
        definition = machine(transitions=(edge("close", Phase.OPEN, Input.CLOSE, Phase.CLOSED, guard=guard),))
        with definition.instantiate(id=1, data=None) as running:
            return running.dispatch(Event(Input.CLOSE, payload)).kind

    def test_all_any_and_not_short_circuit_in_declaration_order(self):
        calls = []
        def leaf(name, value):
            return Condition(name, lambda data, payload, facts: calls.append(name) or value)
        guard = Condition("g", all_of=(leaf("a", True), Condition("any", any_of=(leaf("b", False), leaf("c", True),
            leaf("never-any", True))), Condition("not", negated=leaf("d", False))))
        self.assertEqual(self.evaluate(guard), "transitioned")
        self.assertEqual(calls, ["a", "b", "c", "d"])
        calls.clear()
        self.assertEqual(self.evaluate(Condition("g", all_of=(leaf("a", False), leaf("never-all", True)))), "refused")
        self.assertEqual(calls, ["a"])

    def test_nesting_and_operand_bounds_are_the_kernel_limits(self):
        def nested(depth):
            condition = Condition("leaf", always)
            for level in range(depth - 1):
                condition = Condition(("not", level), negated=condition)
            return condition
        self.assertEqual(self.evaluate(nested(32)), "refused")
        for depth in (33, 200):
            with self.assertRaises(DefinitionLimitError) as caught:
                self.evaluate(nested(depth))
            self.assertEqual(caught.exception.context["reason"], "depth_limit_exceeded")
        operands = (Condition(n, always) for n in count())
        with self.assertRaises(DefinitionLimitError) as caught:
            self.evaluate(Condition("wide", all_of=operands))  # type: ignore[arg-type]
        self.assertEqual((caught.exception.context["reason"], caught.exception.context["condition"]),
            ("operand_limit_exceeded", "wide"))
        with self.assertRaises(InvalidConditionError) as empty:
            self.evaluate(Condition("empty", any_of=()))
        self.assertEqual((empty.exception.context["reason"], empty.exception.context["condition"]), ("empty_group", "empty"))
        with self.assertRaises(DefinitionError):
            self.evaluate(Condition("both", predicate=always, negated=Condition("x", always)))

    def test_predicate_exceptions_and_non_booleans_are_callback_errors_not_false(self):
        failure = ValueError("predicate raised")
        later = []
        def raising(*_):
            raise failure
        with self.assertRaises(CallbackError) as caught:
            self.evaluate(Condition("g", any_of=(Condition("x", raising), Condition("y", lambda *_: later.append(1) or True))))
        self.assertIs(caught.exception.__cause__, failure)
        self.assertEqual(later, [])
        with self.assertRaises(CallbackError) as wrong:
            self.evaluate(Condition("g", lambda *_: 1))
        self.assertIsInstance(wrong.exception.__cause__, TypeError)

class DispositionContractTests(unittest.TestCase):
    """EventDisposition: harmless events, conditions and reasons distinct from refusal."""
    def definition(self, predicate=always, reason="duplicate ping", extra=()):
        return machine(transitions=(edge("close", Phase.OPEN, Input.CLOSE, Phase.CLOSED), *extra),
            dispositions=(Disposition("ping", Phase.OPEN, Input.PING, Condition("harmless", predicate), reason),))

    def test_ignored_is_distinct_from_refused_and_accepts_nothing(self):
        with self.definition(predicate=lambda data, payload, facts: payload == "dup").instantiate(id=1, data=[1]) as running:
            outcome = running.dispatch(Event(Input.PING, "dup"))
            self.assertEqual(outcome, Ignored("ping", Phase.OPEN, Input.PING, "duplicate ping"))
            self.assertEqual((running.state, running.revision, running.data), (Phase.OPEN, 0, [1]))
            self.assertEqual(running.dispatch(Event(Input.PING, "new")), Refused(Phase.OPEN, Input.PING))
            self.assertEqual(running.dispatch(Event(Input.SUBMIT, None)), Refused(Phase.OPEN, Input.SUBMIT))

    def test_reasons_overlaps_and_identities_are_checked(self):
        for reason in ("", "   ", "x" * 257):
            with self.assertRaises(InvalidDispositionError) as caught:
                self.definition(reason=reason)
            self.assertEqual((caught.exception.context["reason"], caught.exception.context["disposition"]),
                ("invalid_reason", "ping"))
        overlapping = self.definition(extra=(edge("ping", Phase.OPEN, Input.PING, Phase.CLOSED),))
        with overlapping.instantiate(id=1, data=None) as running:
            with self.assertRaises(AmbiguousTransitionError) as caught:
                running.dispatch(Event(Input.PING, None))
            self.assertEqual(caught.exception.context["state"], Phase.OPEN)
        with self.assertRaises(DuplicateIdentifierError) as duplicate:
            machine(transitions=(edge("close", Phase.OPEN, Input.CLOSE, Phase.CLOSED),), dispositions=(
                Disposition("p", Phase.OPEN, Input.PING, Condition("c", always), "a"),
                Disposition("p", Phase.OPEN, Input.SUBMIT, Condition("c2", always), "b")))
        self.assertEqual(duplicate.exception.context["disposition"], "p")

class ErrorTaxonomyTests(unittest.TestCase):
    """Every construction rejection is a typed native class carrying the kernel ``reason``."""
    def test_classes_form_one_hierarchy(self):
        import status_machina
        for name in status_machina.__all__:
            value = getattr(status_machina, name)
            if isinstance(value, type) and issubclass(value, Exception):
                self.assertTrue(issubclass(value, StatusMachinaError), name)
        self.assertTrue(issubclass(status_machina.BindingError, DefinitionError))
        self.assertTrue(issubclass(status_machina.UnknownCapabilityError, LookupError))

if __name__ == "__main__":
    unittest.main()
