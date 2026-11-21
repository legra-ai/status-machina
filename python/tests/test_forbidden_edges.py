"""Explicit forbidden-edge constraints through native definitions and portable records."""
import unittest
from enum import Enum, auto
from itertools import count

from status_machina import (
    Action,
    Behavior,
    BehaviorRef,
    BehaviorRegistry,
    BehaviorRole,
    BehaviorUse,
    Condition,
    Definition,
    DefinitionError,
    DefinitionLimitError,
    DefinitionRecord,
    DiagramLabels,
    Disposition,
    DuplicateIdentifierError,
    Event,
    ForbiddenEdge,
    ForbiddenTransitionError,
    Ignored,
    InspectionError,
    InspectionLimits,
    LimitError,
    PredicateRecord,
    RecordError,
    Refused,
    State,
    StateRecord,
    TerminalTransitionError,
    Transition,
    TransitionDefinitionRecord,
    Transitioned,
    UnknownStateError,
)


class Session(Enum):
    OPEN = auto()
    SEALING = auto()
    DONE = auto()
    FAILED = auto()
    CANCELLED = auto()
    GHOST = auto()

class Signal(Enum):
    SEAL = auto()
    FINISH = auto()
    FAIL = auto()
    CANCEL = auto()
    RETRY = auto()
    STALE = auto()

class Story(Enum):
    NEVER = auto()

TERMINAL = (Session.DONE, Session.FAILED, Session.CANCELLED)
STATES = (Session.OPEN, Session.SEALING, *TERMINAL)
SEALING_CANCELLED = ForbiddenEdge(Session.SEALING, Session.CANCELLED)
NEVER = BehaviorRef(Story.NEVER, 1)
REGISTRY = BehaviorRegistry((Behavior(NEVER, BehaviorRole.CONDITION, lambda *_: False),))

def legal_edges():
    return (Transition("seal", Session.OPEN, Signal.SEAL, Session.SEALING),
        Transition("finish", Session.SEALING, Signal.FINISH, Session.DONE),
        Transition("fail", Session.SEALING, Signal.FAIL, Session.FAILED),
        Transition("cancel", Session.OPEN, Signal.CANCEL, Session.CANCELLED))

def session(extra=(), forbidden=(SEALING_CANCELLED,), **options):
    return Definition(id="session", initial=Session.OPEN,
        states=tuple(State(s, terminal=s in TERMINAL) for s in STATES),
        transitions=(*legal_edges(), *extra), forbidden_edges=forbidden, behaviors=REGISTRY, **options)

def record(extra=(), forbidden=(SEALING_CANCELLED,), *, first=False):
    legal = tuple(TransitionDefinitionRecord(t.id, t.source, t.event, t.target) for t in legal_edges())
    return DefinitionRecord(id="session", initial=Session.OPEN,
        states=tuple(StateRecord(s, terminal=s in TERMINAL) for s in STATES),
        transitions=(*extra, *legal) if first else (*legal, *extra), forbidden_edges=tuple(forbidden))

def failure(build):
    try:
        build()
    except DefinitionError as error:
        return type(error), dict(error.context)
    raise AssertionError("the definition was accepted")

CONFLICT = {"reason": "forbidden_transition", "transition": "abandon",
    "source": Session.SEALING, "target": Session.CANCELLED}

class ForbiddenEdgeConstructionTests(unittest.TestCase):
    """STM-16: a transition and a matching prohibition are a contradictory definition."""
    def test_sealing_to_cancelled_is_rejected_in_either_order_for_every_event_and_a_false_guard(self):
        for event in Signal:
            for guard in (None, Condition("never", predicate=lambda *_: False), Condition("never", predicate=NEVER)):
                abandon = Transition("abandon", Session.SEALING, event, Session.CANCELLED, guard=guard)
                with self.subTest(event=event, guard=guard):
                    for build in (lambda abandon=abandon: session((abandon,)),
                            lambda abandon=abandon: Definition(id="session", initial=Session.OPEN, forbidden_edges=(SEALING_CANCELLED,),
                                states=tuple(State(s, terminal=s in TERMINAL) for s in STATES),
                                transitions=(abandon, *legal_edges()), behaviors=REGISTRY)):
                        with self.assertRaises(ForbiddenTransitionError) as caught:
                            build()
                        self.assertIsInstance(caught.exception, DefinitionError)
                        self.assertEqual({key: caught.exception.context[key] for key in CONFLICT}, CONFLICT)

    def test_records_reject_the_same_conflict_with_identical_context(self):
        guard = PredicateRecord(BehaviorUse("never", NEVER))
        for event in Signal:
            for edge_guard in (None, guard):
                abandon = TransitionDefinitionRecord("abandon", Session.SEALING, event, Session.CANCELLED, guard=edge_guard)
                native_guard = None if edge_guard is None else Condition("never", predicate=NEVER)
                native = failure(lambda event=event, native_guard=native_guard: session((Transition(
                    "abandon", Session.SEALING, event, Session.CANCELLED, guard=native_guard),)))
                for first in (False, True):
                    for forbidden in ((SEALING_CANCELLED, ForbiddenEdge(Session.OPEN, Session.DONE)),
                            (ForbiddenEdge(Session.OPEN, Session.DONE), SEALING_CANCELLED)):
                        with self.subTest(event=event, guarded=edge_guard is not None, first=first):
                            with self.assertRaises(RecordError) as caught:
                                record((abandon,), forbidden, first=first).bind(REGISTRY)
                            self.assertEqual(native[0], ForbiddenTransitionError)
                            self.assertEqual(dict(caught.exception.context), native[1])

    def test_unknown_endpoints_duplicates_and_self_transitions_are_typed(self):
        retry = Transition("retry", Session.SEALING, Signal.RETRY, Session.SEALING)
        touch = Transition("retry", Session.SEALING, Signal.RETRY, Session.SEALING, internal=True)
        cases = [
            (lambda: session(forbidden=(ForbiddenEdge(Session.GHOST, Session.CANCELLED),)), UnknownStateError,
                {"reason": "unknown_forbidden_edge_state", "source": Session.GHOST, "target": Session.CANCELLED,
                 "state": Session.GHOST}),
            (lambda: session(forbidden=(ForbiddenEdge(Session.SEALING, Session.GHOST),)), UnknownStateError,
                {"reason": "unknown_forbidden_edge_state", "source": Session.SEALING, "target": Session.GHOST,
                 "state": Session.GHOST}),
            (lambda: session(forbidden=(SEALING_CANCELLED, SEALING_CANCELLED)), DuplicateIdentifierError,
                {"reason": "duplicate_forbidden_edge", "source": Session.SEALING, "target": Session.CANCELLED}),
            (lambda: session((retry,), (ForbiddenEdge(Session.SEALING, Session.SEALING),)), ForbiddenTransitionError,
                {"reason": "forbidden_transition", "transition": "retry", "source": Session.SEALING,
                 "target": Session.SEALING}),
            (lambda: session((touch,), (ForbiddenEdge(Session.SEALING, Session.SEALING),)), ForbiddenTransitionError,
                {"reason": "forbidden_transition", "transition": "retry", "source": Session.SEALING,
                 "target": Session.SEALING}),
        ]
        for build, kind, context in cases:
            with self.subTest(context=context):
                with self.assertRaises(kind) as caught:
                    build()
                self.assertEqual({key: caught.exception.context[key] for key in context}, context)

    def test_record_binding_matches_every_native_outcome(self):
        retry = ("retry", Session.SEALING, Signal.RETRY, Session.SEALING)
        reopen = ("reopen", Session.DONE, Signal.RETRY, Session.SEALING)
        cases = [((), (ForbiddenEdge(Session.GHOST, Session.CANCELLED),)), ((), (SEALING_CANCELLED, SEALING_CANCELLED)),
            ((retry,), (ForbiddenEdge(Session.SEALING, Session.SEALING),)),
            ((reopen,), (ForbiddenEdge(Session.DONE, Session.SEALING),)), ((reopen,), ())]
        for extra, forbidden in cases:
            with self.subTest(extra=extra, forbidden=forbidden):
                native = failure(lambda extra=extra, forbidden=forbidden:
                    session(tuple(Transition(*e) for e in extra), forbidden))
                bound = failure(lambda extra=extra, forbidden=forbidden: record(
                    tuple(TransitionDefinitionRecord(*e) for e in extra), forbidden).bind(REGISTRY))
                self.assertEqual(bound[0], RecordError)
                self.assertEqual(bound[1], native[1])

    def test_terminal_states_keep_rejecting_every_outgoing_edge(self):
        for forbidden in ((), (ForbiddenEdge(Session.DONE, Session.SEALING),)):
            with self.assertRaises(TerminalTransitionError) as caught:
                session((Transition("reopen", Session.DONE, Signal.RETRY, Session.SEALING),), forbidden)
            self.assertEqual(caught.exception.context["state"], Session.DONE)
        self.assertEqual(session(forbidden=(ForbiddenEdge(Session.DONE, Session.OPEN),)).forbidden_edges,
            (ForbiddenEdge(Session.DONE, Session.OPEN),))

    def test_the_bound_is_checked_reading_one_prohibition_beyond_it(self):
        consumed = []
        def endless():
            for index in count():
                consumed.append(index)
                yield ForbiddenEdge(Session.OPEN, Session.DONE)
        with self.assertRaises(DefinitionLimitError) as caught:
            session(forbidden=endless())
        self.assertEqual((caught.exception.context["reason"], caught.exception.context["limit"]),
            ("forbidden_edge_limit_exceeded", 128))
        self.assertEqual(len(consumed), 129)
        with self.assertRaises(LimitError):
            record(forbidden=(SEALING_CANCELLED,) * 129)

    def test_construction_runs_no_callback_and_only_typed_pairs_are_accepted(self):
        calls = []
        abandon = Transition("abandon", Session.SEALING, Signal.CANCEL, Session.CANCELLED,
            guard=Condition("g", predicate=lambda *_: calls.append("guard") or True),
            actions=(Action("a", apply=lambda *_: calls.append("action")),))
        with self.assertRaises(ForbiddenTransitionError):
            session((abandon,))
        self.assertEqual(calls, [])
        with self.assertRaises(DefinitionError):
            session(forbidden=((Session.SEALING, Session.CANCELLED),))  # type: ignore[arg-type]
        with self.assertRaises(RecordError):
            record(forbidden=((Session.SEALING, Session.CANCELLED),))  # type: ignore[arg-type]


class ForbiddenEdgeBehaviorTests(unittest.TestCase):
    """Prohibitions are structural only: dispatch keeps default refusal and dispositions."""
    def definition(self):
        return session(dispositions=(Disposition("stale", Session.SEALING, Signal.STALE,
            Condition("always", predicate=lambda *_: True), "stale completion"),))

    def test_legal_edges_dispatch_and_the_forbidden_event_refuses_without_change(self):
        definition = self.definition()
        for signal, target in ((Signal.FINISH, Session.DONE), (Signal.FAIL, Session.FAILED)):
            with definition.instantiate(id=1, data={"n": 1}) as running:
                self.assertIsInstance(running.dispatch(Event(Signal.SEAL, None)), Transitioned)
                self.assertEqual(running.dispatch(Event(Signal.CANCEL, None)), Refused(Session.SEALING, Signal.CANCEL))
                self.assertEqual((running.state, running.revision, running.data), (Session.SEALING, 1, {"n": 1}))
                self.assertEqual(running.dispatch(Event(Signal.STALE, None)),
                    Ignored("stale", Session.SEALING, Signal.STALE, "stale completion"))
                self.assertEqual(running.revision, 1)
                outcome = running.dispatch(Event(signal, None))
                self.assertIsInstance(outcome, Transitioned)
                self.assertEqual(running.state, target)

    def test_record_bound_definitions_expose_and_enforce_the_same_pairs(self):
        bound = record(forbidden=(SEALING_CANCELLED,)).bind(REGISTRY)
        self.assertEqual(bound.forbidden_edges, (SEALING_CANCELLED,))
        self.assertEqual(bound.record.forbidden_edges, (SEALING_CANCELLED,))


class ForbiddenEdgeInspectionTests(unittest.TestCase):
    PAIRS = (SEALING_CANCELLED, ForbiddenEdge(Session.OPEN, Session.DONE), ForbiddenEdge(Session.SEALING, Session.SEALING))
    LABELS = DiagramLabels(states={s: s.name for s in STATES},
        transitions={t.id: t.id for t in legal_edges()}, events={s: s.name for s in Signal})

    def test_coverage_lists_typed_pairs_independent_of_declaration_order(self):
        forward = session(forbidden=self.PAIRS).structural_coverage()
        reversed_ = session(forbidden=tuple(reversed(self.PAIRS))).structural_coverage()
        self.assertEqual(forward, reversed_)
        self.assertEqual(forward.forbidden_edges, (ForbiddenEdge(Session.OPEN, Session.DONE),
            ForbiddenEdge(Session.SEALING, Session.SEALING), SEALING_CANCELLED))
        plain = session(forbidden=()).structural_coverage()
        self.assertEqual((forward.states, forward.transitions), (plain.states, plain.transitions))
        self.assertEqual(plain.forbidden_edges, ())
        # Five states, four transitions and three prohibitions.
        self.assertEqual(len(session(forbidden=self.PAIRS).structural_coverage(
            limits=InspectionLimits(max_entries=12)).forbidden_edges), 3)
        with self.assertRaises(InspectionError) as caught:
            session(forbidden=self.PAIRS).structural_coverage(limits=InspectionLimits(max_entries=11))
        self.assertEqual(caught.exception.context["limit"], 11)

    def test_diagrams_never_render_prohibitions(self):
        self.assertEqual(session(forbidden=self.PAIRS).diagram(self.LABELS),
            session(forbidden=()).diagram(self.LABELS))
        self.assertEqual(session(forbidden=self.PAIRS).diagram(self.LABELS, limits=InspectionLimits(max_entries=9)),
            session(forbidden=()).diagram(self.LABELS))
