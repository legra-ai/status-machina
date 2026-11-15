"""Portable records bind exact Story implementations through the kernel record API."""
from dataclasses import dataclass
from enum import Enum, auto
import unittest
from status_machina import (
    AllRecord, AnyRecord, Behavior, BehaviorRef, BehaviorRegistry, BehaviorRole, BehaviorUse,
    BindingError, CallbackError, ConditionRecordError, DefinitionError, DefinitionRecord,
    DispositionRecord, DispositionRecordError, Event, LimitError, NotRecord, PredicateRecord,
    RecordError, RoleMismatchError, StateRecord, TransitionDefinitionRecord, TransitionRecordError,
    UnknownBehaviorError, UnknownImplementationError,
)

class Phase(Enum):
    DRAFT = auto()
    SENT = auto()
    PAID = auto()

class Input(Enum):
    SEND = auto()
    PAY = auto()
    NUDGE = auto()
    SENT_NOTICE = auto()

class Story(Enum):
    HAS_LINES = auto()
    STAMP = auto()
    NOTIFY = auto()
    ANNOUNCE = auto()
    ALWAYS = auto()

class Version(Enum):
    V1 = auto()
    V2 = auto()

class Edge(Enum):
    SEND = auto()
    PAY = auto()

class Use(Enum):
    GUARD = auto()
    STAMP = auto()
    NOTIFY = auto()
    IGNORE = auto()

@dataclass
class Invoice:
    lines: int
    stamps: int = 0

HAS_LINES = BehaviorRef(Story.HAS_LINES, Version.V1)
STAMP = BehaviorRef(Story.STAMP, Version.V1)
NOTIFY = BehaviorRef(Story.NOTIFY, Version.V1)
ANNOUNCE = BehaviorRef(Story.ANNOUNCE, Version.V1)
ALWAYS = BehaviorRef(Story.ALWAYS, Version.V1)

def stamp(data, payload, facts):
    data.stamps += 1

def registry(*extra):
    return BehaviorRegistry((
        Behavior(HAS_LINES, BehaviorRole.CONDITION, lambda data, payload, facts: data.lines > 0),
        Behavior(STAMP, BehaviorRole.ACTION, stamp),
        Behavior(NOTIFY, BehaviorRole.EFFECT, lambda data, payload, facts: ("notify", data.lines)),
        Behavior(ANNOUNCE, BehaviorRole.OUTPUT, lambda data, payload, facts: data.stamps),
        Behavior(ALWAYS, BehaviorRole.CONDITION, lambda data, payload, facts: True),
        *extra))

def record(*, guard=None, transitions=None, states=None, dispositions=()):
    guard = PredicateRecord(BehaviorUse(Use.GUARD, HAS_LINES)) if guard is None else guard
    states = (StateRecord(Phase.DRAFT), StateRecord(Phase.SENT),
        StateRecord(Phase.PAID, terminal=True)) if states is None else states
    transitions = (
        TransitionDefinitionRecord(Edge.SEND, Phase.DRAFT, Input.SEND, Phase.SENT, guard=guard,
            actions=(BehaviorUse(Use.STAMP, STAMP),), effects=(BehaviorUse(Use.NOTIFY, NOTIFY),),
            outputs=(BehaviorUse(Input.SENT_NOTICE, ANNOUNCE),)),
        TransitionDefinitionRecord(Edge.PAY, Phase.SENT, Input.PAY, Phase.PAID),
    ) if transitions is None else transitions
    return DefinitionRecord(id=object(), initial=Phase.DRAFT, states=states,
        transitions=transitions, dispositions=dispositions)

class RecordBindingTests(unittest.TestCase):
    def test_bound_record_runs_registered_implementations_with_native_identities(self):
        nudge = DispositionRecord(Use.IGNORE, Phase.SENT, Input.NUDGE,
            PredicateRecord(BehaviorUse(Use.IGNORE, ALWAYS)), "reminders are harmless")
        source = record(dispositions=(nudge,))
        definition = source.bind(registry())
        self.assertIs(definition.record, source)
        self.assertIs(definition.initial, Phase.DRAFT)
        self.assertEqual([state.id for state in definition.states], list(Phase))
        with definition.instantiate(id=object(), data=Invoice(lines=2)) as machine:
            outcome = machine.dispatch(Event(Input.SEND, None))
            self.assertEqual(outcome.kind, "transitioned")
            self.assertIs(outcome.record.transition, Edge.SEND)
            self.assertIs(outcome.effects[0].id, Use.NOTIFY)
            self.assertEqual(outcome.effects[0].payload, ("notify", 2))
            self.assertIs(outcome.outputs[0].kind, Input.SENT_NOTICE)
            self.assertEqual(outcome.outputs[0].payload, 1)
            self.assertEqual(machine.data, Invoice(lines=2, stamps=1))
            ignored = machine.dispatch(Event(Input.NUDGE, None))
            self.assertEqual((ignored.kind, ignored.disposition, ignored.reason),
                ("ignored", Use.IGNORE, "reminders are harmless"))
            self.assertIs(machine.dispatch(Event(Input.PAY, None)).record.target, Phase.PAID)

    def test_condition_records_short_circuit_in_declared_order(self):
        calls = []
        never = BehaviorRef(Story.ALWAYS, Version.V2)
        behaviors = registry(Behavior(never, BehaviorRole.CONDITION,
            lambda *args: calls.append("never") or False))
        guard = AnyRecord(object(), (PredicateRecord(BehaviorUse(object(), ALWAYS)),
            PredicateRecord(BehaviorUse(object(), never))))
        guard = AllRecord(object(), (NotRecord(object(), NotRecord(object(), guard)),))
        with record(guard=guard).bind(behaviors).instantiate(id=object(), data=Invoice(1)) as machine:
            self.assertEqual(machine.dispatch(Event(Input.SEND, None)).kind, "transitioned")
        self.assertEqual(calls, [])

    def test_bound_callback_failure_keeps_its_original_cause(self):
        failure = ValueError("ledger unavailable")
        def broken(*args):
            raise failure
        flaky = BehaviorRef(Story.HAS_LINES, Version.V2)
        behaviors = registry(Behavior(flaky, BehaviorRole.CONDITION, broken))
        definition = record(guard=PredicateRecord(BehaviorUse(Use.GUARD, flaky))).bind(behaviors)
        with definition.instantiate(id=object(), data=Invoice(1)) as machine:
            with self.assertRaises(CallbackError) as caught:
                machine.dispatch(Event(Input.SEND, None))
            self.assertIs(caught.exception.__cause__, failure)
            self.assertIs(machine.state, Phase.DRAFT)

    def test_records_reject_callables_and_foreign_values_at_construction(self):
        with self.assertRaises(RecordError):
            BehaviorUse(Use.STAMP, stamp)
        with self.assertRaises(RecordError):
            StateRecord(Phase.DRAFT, invariant=lambda data: True)
        with self.assertRaises(RecordError):
            StateRecord(Phase.DRAFT, entry_actions=(STAMP,))
        with self.assertRaises(RecordError):
            AllRecord(object(), (HAS_LINES,))
        with self.assertRaises(RecordError):
            BehaviorUse([], STAMP)
        with self.assertRaises(RecordError):
            DefinitionRecord(id=object(), initial=Phase.DRAFT, states=(Phase.DRAFT,), transitions=())
        with self.assertRaises(LimitError):
            AnyRecord(object(), [PredicateRecord(BehaviorUse(object(), ALWAYS))] * 17)
        with self.assertRaises(LimitError):
            DefinitionRecord(id=object(), initial=Phase.DRAFT, max_states=0, states=(), transitions=())
        with self.assertRaises(DefinitionError):
            record().bind((Behavior(STAMP, BehaviorRole.ACTION, stamp),))

    def test_story_resolution_never_substitutes_or_loads_code(self):
        missing = BehaviorRef("os.system", Version.V1)
        with self.assertRaises(UnknownBehaviorError) as caught:
            record(guard=PredicateRecord(BehaviorUse(Use.GUARD, missing))).bind(registry())
        self.assertEqual(caught.exception.context["story"], "os.system")
        self.assertIsInstance(caught.exception, BindingError)
        other = BehaviorRef(Story.HAS_LINES, Version.V2)
        with self.assertRaises(UnknownImplementationError) as caught:
            record(guard=PredicateRecord(BehaviorUse(Use.GUARD, other))).bind(registry())
        self.assertIs(caught.exception.context["story"], Story.HAS_LINES)
        self.assertIs(caught.exception.context["implementation"], Version.V2)
        with self.assertRaises(RoleMismatchError) as caught:
            record(guard=PredicateRecord(BehaviorUse(Use.GUARD, STAMP))).bind(registry())
        self.assertIs(caught.exception.context["expected"], BehaviorRole.CONDITION)
        self.assertIs(caught.exception.context["actual"], BehaviorRole.ACTION)

    def test_structural_failures_name_the_native_record(self):
        deep = PredicateRecord(BehaviorUse("innermost", ALWAYS))
        for depth in range(32):
            deep = NotRecord(depth, deep)
        with self.assertRaises(ConditionRecordError) as caught:
            record(guard=deep).bind(registry())
        self.assertEqual(caught.exception.context["condition"], "innermost")
        with self.assertRaises(ConditionRecordError) as caught:
            record(guard=AllRecord("empty", ())).bind(registry())
        self.assertEqual(caught.exception.context["condition"], "empty")
        internal = (TransitionDefinitionRecord(Edge.SEND, Phase.DRAFT, Input.SEND, Phase.SENT, internal=True),
            TransitionDefinitionRecord(Edge.PAY, Phase.SENT, Input.PAY, Phase.PAID))
        with self.assertRaises(TransitionRecordError) as caught:
            record(transitions=internal).bind(registry())
        self.assertIs(caught.exception.context["transition"], Edge.SEND)
        noisy = DispositionRecord(Use.IGNORE, Phase.SENT, Input.NUDGE,
            PredicateRecord(BehaviorUse(Use.IGNORE, ALWAYS)), "x" * 257)
        with self.assertRaises(DispositionRecordError) as caught:
            record(dispositions=(noisy,)).bind(registry())
        self.assertIs(caught.exception.context["disposition"], Use.IGNORE)

    def test_graph_rules_report_native_identities(self):
        orphan = (StateRecord(Phase.DRAFT), StateRecord(Phase.SENT), StateRecord(Phase.PAID, terminal=True))
        only_send = (TransitionDefinitionRecord(Edge.SEND, Phase.DRAFT, Input.SEND, Phase.SENT),)
        with self.assertRaises(RecordError) as caught:
            record(states=orphan, transitions=only_send).bind(registry())
        self.assertIs(caught.exception.context["state"], Phase.PAID)
        dangling = (TransitionDefinitionRecord(Edge.SEND, Phase.DRAFT, Input.SEND, Phase.PAID),)
        with self.assertRaises(RecordError) as caught:
            record(states=(StateRecord(Phase.DRAFT),), transitions=dangling).bind(registry())
        self.assertIs(caught.exception.context["transition"], Edge.SEND)
        self.assertIs(caught.exception.context["state"], Phase.PAID)
