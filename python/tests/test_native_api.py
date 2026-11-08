"""Applications use native enums, dataclasses, exceptions and owned snapshots."""
from dataclasses import dataclass
from enum import Enum, IntEnum, auto
import unittest
from status_machina import (
    Action, CallbackError, Condition, Definition, DefinitionError, DisposedError,
    Effect, Event, InvariantError, State, Transition,
)

class Phase(Enum):
    OPEN = auto()
    CLOSED = auto()

class Input(Enum):
    CLOSE = auto()
    UNKNOWN = auto()

@dataclass
class Counter:
    values: list[int]

class NativeApiTests(unittest.TestCase):
    def definition(self, *, actions=(), guard=None, invariant=None, effects=()):
        return Definition(
            id=object(), initial=Phase.OPEN,
            states=(State(Phase.OPEN), State(Phase.CLOSED, invariant=invariant, terminal=True)),
            transitions=(Transition(id=object(), source=Phase.OPEN, event=Input.CLOSE,
                target=Phase.CLOSED, guard=guard, actions=actions, effects=effects),),
        )

    def test_native_enum_and_dataclass_survive_dispatch_and_inspection(self):
        def update(data, event, facts):
            data.values.append(event + facts)
        definition = self.definition(actions=(Action(id=object(), apply=update),))
        original = Counter([1])
        with definition.instantiate(id=object(), data=original) as machine:
            result = machine.dispatch(Event(Input.CLOSE, 2), facts=3)
            self.assertEqual(result.kind, "transitioned")
            self.assertIs(machine.state, Phase.CLOSED)
            self.assertIsInstance(machine.data, Counter)
            self.assertEqual(machine.data.values, [1, 5])
            self.assertEqual(original.values, [1])
            inspected = machine.data
            inspected.values.append(99)
            self.assertEqual(machine.data.values, [1, 5])
        with self.assertRaises(DisposedError):
            _ = machine.state

    def test_guard_exception_keeps_original_cause_and_never_becomes_refusal(self):
        failure = ValueError("domain guard failed")
        calls = []
        def predicate(data, event, facts):
            data.values.append(99)
            raise failure
        definition = self.definition(guard=Condition(id=object(), predicate=predicate),
            actions=(Action(id=object(), apply=lambda *args: calls.append(True)),))
        with definition.instantiate(id=object(), data=Counter([1])) as machine:
            with self.assertRaises(CallbackError) as caught:
                machine.dispatch(Event(Input.CLOSE, None), facts=None)
            self.assertIs(caught.exception.__cause__, failure)
            self.assertIs(machine.state, Phase.OPEN)
            self.assertEqual(machine.data.values, [1])
            self.assertEqual(machine.revision, 0)
            self.assertEqual(calls, [])

    def test_retained_action_argument_cannot_mutate_accepted_data(self):
        retained = []
        def action(data, event, facts):
            retained.append(data)
            data.values.append(2)
        with self.definition(actions=(Action(id=object(), apply=action),)).instantiate(
                id=object(), data=Counter([1])) as machine:
            machine.dispatch(Event(Input.CLOSE, None), facts=None)
            retained[0].values.append(99)
            self.assertEqual(machine.data.values, [1, 2])

    def test_refusal_and_invariant_rejection_leave_state_unchanged(self):
        definition = self.definition(invariant=lambda data: bool(data.values))
        with definition.instantiate(id=object(), data=Counter([])) as machine:
            self.assertEqual(machine.dispatch(Event(Input.UNKNOWN, None), facts=None).kind, "refused")
            with self.assertRaises(InvariantError):
                machine.dispatch(Event(Input.CLOSE, None), facts=None)
            self.assertIs(machine.state, Phase.OPEN)
            self.assertEqual(machine.revision, 0)

    def test_intenum_roles_do_not_alias_by_their_underlying_number(self):
        class Other(IntEnum):
            CLOSE = 1
        class Expected(IntEnum):
            CLOSE = 1
        definition = Definition(id=object(), initial=Phase.OPEN,
            states=(State(Phase.OPEN), State(Phase.CLOSED)),
            transitions=(Transition(id=object(), source=Phase.OPEN,
                event=Expected.CLOSE, target=Phase.CLOSED),))
        with definition.instantiate(id=object(), data=None) as machine:
            self.assertEqual(machine.dispatch(Event(Other.CLOSE, None), facts=None).kind, "refused")
            self.assertEqual(machine.dispatch(Event(Expected.CLOSE, None), facts=None).kind, "transitioned")

    def test_invalid_definition_fails_before_instantiation(self):
        with self.assertRaises(DefinitionError):
            Definition(id=object(), initial=Phase.OPEN, states=(State(Phase.OPEN),),
                transitions=(Transition(id=object(), source=Phase.OPEN,
                    event=Input.CLOSE, target=Phase.CLOSED),))

if __name__ == "__main__":
    unittest.main()
