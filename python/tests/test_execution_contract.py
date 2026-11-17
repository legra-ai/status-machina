"""Actions, intents, Story bindings, instances, decisions, outcomes, capabilities and EventOnDrop, row by row."""
import unittest
from dataclasses import dataclass, field
from enum import Enum, auto

from status_machina import (
    Action,
    Behavior,
    BehaviorRef,
    BehaviorRegistry,
    BehaviorRole,
    CallbackError,
    Capability,
    CapabilityStatus,
    Condition,
    Definition,
    DefinitionError,
    DefinitionLimitError,
    DisposedError,
    DuplicateIdentifierError,
    DuplicateImplementationError,
    Effect,
    EffectIntent,
    Event,
    EventOnDrop,
    InvariantError,
    OutputEvent,
    OutputIntent,
    PostconditionError,
    Refused,
    RoleMismatchError,
    StaleDecisionError,
    State,
    Transition,
    Transitioned,
    TransitionRecord,
    UnknownBehaviorError,
    UnknownCapabilityError,
    UnknownImplementationError,
)


class Phase(Enum):
    DRAFT = auto()
    SENT = auto()

class Input(Enum):
    SEND = auto()
    OTHER = auto()

class Story(Enum):
    STAMP = auto()
    POSITIVE = auto()
    NOTICE = auto()
    VALID = auto()

@dataclass
class Invoice:
    total: int
    log: list[str] = field(default_factory=list)

@dataclass(frozen=True)
class Notice:
    total: int

def stamp(data, payload, facts):
    data.log.append("stamp")

def definition(*, actions=(), effects=(), outputs=(), guard=None, postconditions=(), invariant=None,
        behaviors=BehaviorRegistry(), capabilities=(), entry=(), exit=()):
    return Definition(id="invoices", initial=Phase.DRAFT, states=(State(Phase.DRAFT, exit_actions=exit),
        State(Phase.SENT, invariant=invariant, entry_actions=entry)), transitions=(Transition(id="send",
            source=Phase.DRAFT, event=Input.SEND, target=Phase.SENT, guard=guard, postconditions=postconditions,
            actions=actions, effects=effects, outputs=outputs),), behaviors=behaviors, capabilities=capabilities)

class IntentContractTests(unittest.TestCase):
    """LocalAction, Effect and OutputEvent: phase order, owned payloads and rollback."""
    def test_actions_run_in_phase_order_and_intents_keep_native_payloads(self):
        def named(name):
            return Action(id=name, apply=lambda data, payload, facts: data.log.append(name))
        made = definition(exit=(named("exit"),), actions=(named("a1"), named("a2")), entry=(named("entry"),),
            effects=(Effect(id="mail", describe=lambda data, payload, facts: Notice(data.total)),),
            outputs=(OutputEvent(kind=Input.OTHER, describe=lambda data, payload, facts: Notice(facts)),))
        with made.instantiate(id=1, data=Invoice(5)) as running:
            outcome = running.dispatch(Event(Input.SEND, None), facts=9)
            self.assertEqual(running.data.log, ["exit", "a1", "a2", "entry"])
            assert isinstance(outcome, Transitioned)
            self.assertEqual(outcome.effects, (EffectIntent("mail", Notice(5)),))
            self.assertEqual(outcome.outputs, (OutputIntent(Input.OTHER, Notice(9)),))
            self.assertIs(type(outcome.effects[0].payload), Notice)

    def test_functional_update_replaces_candidate_data(self):
        made = definition(actions=(Action(id="double", update=lambda data, payload, facts: Invoice(data.total * 2)),))
        with made.instantiate(id=1, data=Invoice(4)) as running:
            running.dispatch(Event(Input.SEND, None))
            self.assertEqual(running.data, Invoice(8))
        with self.assertRaises(DefinitionError):
            definition(actions=(Action(id="both", apply=stamp, update=lambda *_: None),))
        with self.assertRaises(DefinitionError):
            definition(actions=(Action(id="neither"),))

    def test_any_failing_phase_rolls_back_the_whole_transition(self):
        failure = RuntimeError("constructor raised")
        def raising(*_):
            raise failure
        cases = [
            ({"actions": (Action(id="stamp", apply=stamp), Action(id="boom", apply=raising))}, CallbackError),
            ({"actions": (Action(id="returns", apply=lambda *_: 1),)}, CallbackError),
            ({"actions": (Action(id="stamp", apply=stamp),), "effects": (Effect(id="e", describe=raising),)}, CallbackError),
            ({"actions": (Action(id="stamp", apply=stamp),), "outputs": (OutputEvent(kind="o", describe=raising),)}, CallbackError),
            ({"actions": (Action(id="stamp", apply=stamp),), "postconditions": (Condition("p", lambda *_: False),)},
                PostconditionError),
            ({"actions": (Action(id="stamp", apply=stamp),), "invariant": lambda data: False}, InvariantError),
        ]
        for spec, kind in cases:
            with self.subTest(kind=kind.__name__, spec=sorted(spec)), \
                    definition(**spec).instantiate(id=1, data=Invoice(1)) as running:
                with self.assertRaises(kind):
                    running.dispatch(Event(Input.SEND, None))
                self.assertEqual((running.state, running.revision, running.data.log), (Phase.DRAFT, 0, []))
        with self.assertRaises(DefinitionLimitError):
            definition(effects=tuple(Effect(id=n, describe=raising) for n in range(17)))

class StoryBindingContractTests(unittest.TestCase):
    """BehaviorRef, the registry, roles and BoundInvariant."""
    def registry(self, *behaviors):
        return BehaviorRegistry(tuple(Behavior(BehaviorRef(story, "v1"), role, callback) for story, role, callback in behaviors))

    def test_every_role_resolves_the_exact_story_and_implementation(self):
        registry = self.registry((Story.STAMP, BehaviorRole.ACTION, stamp),
            (Story.POSITIVE, BehaviorRole.CONDITION, lambda data, payload, facts: data.total > 0),
            (Story.NOTICE, BehaviorRole.EFFECT, lambda data, payload, facts: Notice(data.total)),
            (Story.VALID, BehaviorRole.INVARIANT, lambda data: "stamp" in data.log))
        made = definition(behaviors=registry, actions=(Action(id="stamp", apply=BehaviorRef(Story.STAMP, "v1")),),
            guard=Condition("positive", BehaviorRef(Story.POSITIVE, "v1")),
            effects=(Effect(id="notice", describe=BehaviorRef(Story.NOTICE, "v1")),),
            invariant=BehaviorRef(Story.VALID, "v1"))
        with made.instantiate(id=1, data=Invoice(3)) as running:
            outcome = running.dispatch(Event(Input.SEND, None))
            assert isinstance(outcome, Transitioned)
            self.assertEqual(outcome.effects, (EffectIntent("notice", Notice(3)),))
        with made.instantiate(id=2, data=Invoice(0)) as refused:
            self.assertEqual(refused.dispatch(Event(Input.SEND, None)).kind, "refused")

    def test_bound_invariant_rejects_candidates(self):
        registry = self.registry((Story.VALID, BehaviorRole.INVARIANT, lambda data: data.total < 0))
        with definition(behaviors=registry, invariant=BehaviorRef(Story.VALID, "v1")).instantiate(id=1,
                data=Invoice(1)) as running:
            with self.assertRaises(InvariantError) as caught:
                running.dispatch(Event(Input.SEND, None))
            self.assertEqual(caught.exception.context["state"], Phase.SENT)

    def test_unresolved_references_are_typed_binding_errors(self):
        registry = self.registry((Story.STAMP, BehaviorRole.ACTION, stamp))
        with self.assertRaises(UnknownBehaviorError) as unknown:
            definition(behaviors=registry, actions=(Action(id="a", apply=BehaviorRef(Story.NOTICE, "v1")),))
        self.assertEqual((unknown.exception.context["story"], unknown.exception.context["action"]), (Story.NOTICE, "a"))
        with self.assertRaises(UnknownImplementationError) as version:
            definition(behaviors=registry, actions=(Action(id="a", apply=BehaviorRef(Story.STAMP, "v2")),))
        self.assertEqual(version.exception.context["implementation"], "v2")
        with self.assertRaises(RoleMismatchError) as role:
            definition(behaviors=registry, guard=Condition("g", BehaviorRef(Story.STAMP, "v1")))
        self.assertEqual((role.exception.context["expected"], role.exception.context["actual"],
            role.exception.context["condition"]), (BehaviorRole.CONDITION, BehaviorRole.ACTION, "g"))
        with self.assertRaises(RoleMismatchError):
            definition(behaviors=registry, invariant=BehaviorRef(Story.STAMP, "v1"))
        with self.assertRaises(DuplicateImplementationError) as duplicate:
            definition(behaviors=self.registry((Story.STAMP, BehaviorRole.ACTION, stamp),
                (Story.STAMP, BehaviorRole.ACTION, stamp)))
        self.assertEqual(duplicate.exception.context["story"], Story.STAMP)
        with self.assertRaises(DefinitionError):
            definition(behaviors=registry, actions=(Action(id="a", update=BehaviorRef(Story.STAMP, "v1")),))  # type: ignore[arg-type]

class InstanceContractTests(unittest.TestCase):
    """MachineInstance, Event, decisions, outcomes and records."""
    def test_outcomes_and_records_retain_native_identities_and_revisions(self):
        with definition().instantiate(id=("tenant", 7), data=Invoice(1)) as running:
            self.assertEqual(running.id, ("tenant", 7))
            self.assertEqual(running.dispatch(Event(Input.OTHER, None)), Refused(Phase.DRAFT, Input.OTHER))
            outcome = running.dispatch(Event(Input.SEND, None))
            self.assertEqual(outcome, Transitioned(TransitionRecord("send", Phase.DRAFT, Phase.SENT, 0, 1), (), ()))
            self.assertEqual((running.state, running.revision), (Phase.SENT, 1))

    def test_explicit_revisions_and_single_use_decisions(self):
        with definition(actions=(Action(id="stamp", apply=stamp),)).instantiate(id=1, data=Invoice(1)) as running:
            with self.assertRaises(StaleDecisionError) as stale:
                running.dispatch(Event(Input.SEND, None), expected_revision=4)
            self.assertEqual((stale.exception.context["expected_revision"], stale.exception.context["current_revision"]), (4, 0))
            decision = running.evaluate(Event(Input.SEND, None), expected_revision=0)
            self.assertEqual((decision.expected_revision, running.state, running.data.log), (0, Phase.DRAFT, []))
            self.assertEqual(running.accept(decision).kind, "transitioned")
            self.assertEqual(running.data.log, ["stamp"])
            with self.assertRaises(DisposedError):
                running.accept(decision)
            discarded = running.evaluate(Event(Input.OTHER, None))
            discarded.dispose()
            discarded.dispose()
            with self.assertRaises(DisposedError):
                _ = discarded.expected_revision
        running.dispose()
        for operation in (lambda: running.state, lambda: running.data, lambda: running.dispatch(Event(Input.SEND, None))):
            with self.assertRaises(DisposedError):
                operation()

class CapabilityContractTests(unittest.TestCase):
    """Capability values: advisory, exact identities, typed rejections."""
    def test_capability_identities_are_checked(self):
        made = definition(capabilities=(Capability("edit", lambda state, data, facts: CapabilityStatus.AVAILABLE),))
        with made.instantiate(id=1, data=Invoice(1)) as running:
            report = running.evaluate_capability("edit", facts=None)
            self.assertEqual((report.capability, report.instance, report.revision, report.status),
                ("edit", 1, 0, CapabilityStatus.AVAILABLE))
            with self.assertRaises(UnknownCapabilityError) as unknown:
                running.evaluate_capability("delete", facts=None)
            self.assertEqual(unknown.exception.context["capability"], "delete")
            self.assertIsInstance(unknown.exception, LookupError)
        with self.assertRaises(DuplicateIdentifierError) as duplicate:
            definition(capabilities=(Capability("edit", lambda *_: CapabilityStatus.UNKNOWN),
                Capability("edit", lambda *_: CapabilityStatus.UNKNOWN)))
        self.assertEqual((duplicate.exception.context["reason"], duplicate.exception.context["capability"]),
            ("duplicate_capability", "edit"))

class EventOnDropContractTests(unittest.TestCase):
    """EventOnDrop: one event on scope exit unless disarmed after explicit completion."""
    def test_unfinished_scope_delivers_its_event_once(self):
        made = definition()
        with made.instantiate(id=1, data=Invoice(1)) as running:
            delivered = []
            def abandon(event):
                delivered.append(running.dispatch(event).kind)
            with self.assertRaises(KeyError), EventOnDrop(Event(Input.OTHER, None), abandon) as guard:
                guard.event = Event(Input.SEND, "updated")
                raise KeyError("operation failed")
            self.assertEqual((delivered, running.state, guard.armed), (["transitioned"], Phase.SENT, False))

    def test_completed_scope_disarms_and_cannot_rearm(self):
        delivered = []
        with EventOnDrop("abandoned", delivered.append) as guard:
            self.assertEqual(guard.event, "abandoned")
            guard.disarm()
            guard.disarm()
        self.assertEqual((delivered, guard.event), ([], None))
        with self.assertRaises(DisposedError):
            guard.event = "again"
        with self.assertRaises(DisposedError), guard:
            pass
        EventOnDrop("never entered", delivered.append)
        self.assertEqual(delivered, [])

    def test_handlers_are_synchronous_and_return_none(self):
        async def asynchronous(event):
            return None
        with self.assertRaises(TypeError):
            EventOnDrop("x", asynchronous)
        with self.assertRaises(TypeError):
            EventOnDrop("x", "not callable")  # type: ignore[arg-type]
        with self.assertRaises(TypeError), EventOnDrop("x", lambda event: "unhandled"):  # type: ignore[arg-type, return-value]
            pass
        failure = ValueError("handler raised")
        def raising(event):
            raise failure
        with self.assertRaises(ValueError) as caught, EventOnDrop("x", raising):
            raise KeyError("original")
        self.assertIs(caught.exception, failure)
        self.assertIsInstance(caught.exception.__context__, KeyError)

if __name__ == "__main__":
    unittest.main()
