"""Identity codecs are explicit, role-scoped and validated by the kernel IdentityMap."""
from dataclasses import dataclass
from enum import Enum, IntEnum, auto
import itertools
import unittest
from status_machina import (
    BehaviorRef, BehaviorRegistry, Behavior, BehaviorRole, BehaviorUse, DefinitionRecord,
    DuplicateIdentityError, DuplicateRepresentationError, Event, IdentityMap, IdentityMapError,
    IdentityMapLimitError, IdentityRole, IdentityRoleError, LimitError, MachineIdentityMaps,
    StateRecord, TransitionDefinitionRecord, UnknownIdentityError, UnknownRepresentationError,
)

class Phase(Enum):
    OPEN = "open"
    CLOSED = "closed"

class Input(Enum):
    CLOSE = auto()

class Priority(IntEnum):
    LOW = 1

class Severity(IntEnum):
    LOW = 1

@dataclass(frozen=True)
class OrderId:
    value: int

@dataclass(frozen=True)
class InvoiceId:
    value: int

class IdentityMapTests(unittest.TestCase):
    def test_hashable_newtypes_compare_by_value_within_their_type(self):
        instances = IdentityMap(IdentityRole.INSTANCE, ((OrderId(7), "order/7"), (InvoiceId(7), "invoice/7")))
        self.assertEqual(instances.decode("order/7"), OrderId(7))
        self.assertIsInstance(instances.decode("invoice/7"), InvoiceId)
        self.assertEqual(instances.encode(OrderId(7)), "order/7")
        with self.assertRaises(UnknownIdentityError):
            instances.encode(OrderId(8))


    def test_round_trip_returns_the_original_native_values(self):
        states = IdentityMap(IdentityRole.STATE, ((Phase.OPEN, "ex:Open"), (Phase.CLOSED, "ex:Closed")))
        self.assertIs(states.role, IdentityRole.STATE)
        self.assertEqual(len(states), 2)
        self.assertEqual(states.encode(Phase.OPEN), "ex:Open")
        self.assertIs(states.decode("ex:Closed"), Phase.CLOSED)
        self.assertEqual(list(states), [(Phase.OPEN, "ex:Open"), (Phase.CLOSED, "ex:Closed")])
        token = object()
        tokens = IdentityMap(IdentityRole.DEFINITION, ((token, 7),))
        self.assertIs(tokens.decode(7), token)
        with self.assertRaises(UnknownIdentityError):
            tokens.encode(object())

    def test_keys_include_their_concrete_type(self):
        mixed = IdentityMap(IdentityRole.EVENT, ((Priority.LOW, "priority"), (Severity.LOW, "severity"),
            (1, "one"), (True, "true")))
        self.assertEqual([mixed.encode(v) for v in (Priority.LOW, Severity.LOW, 1, True)],
            ["priority", "severity", "one", "true"])
        numbers = IdentityMap(IdentityRole.STATE, ((Phase.OPEN, 1), (Phase.CLOSED, True)))
        self.assertIs(numbers.decode(True), Phase.CLOSED)
        self.assertIs(numbers.decode(1), Phase.OPEN)

    def test_no_label_or_value_derived_codec(self):
        states = IdentityMap(IdentityRole.STATE, ((Phase.OPEN, "ex:Open"),))
        for value in ("open", "Phase.OPEN", "ex:Open"):
            with self.assertRaises(UnknownIdentityError) as caught:
                states.encode(value)
            self.assertIsInstance(caught.exception, LookupError)
            self.assertIs(caught.exception.context["role"], IdentityRole.STATE)
        for value in ("open", Phase.OPEN):
            with self.assertRaises(UnknownRepresentationError):
                states.decode(value)

    def test_duplicates_name_the_offending_entry(self):
        with self.assertRaises(DuplicateIdentityError) as caught:
            IdentityMap(IdentityRole.STATE, ((Phase.OPEN, "a"), (Phase.CLOSED, "b"), (Phase.OPEN, "c")))
        self.assertEqual(caught.exception.context["entry"], 2)
        self.assertIs(caught.exception.context["identity"], Phase.OPEN)
        with self.assertRaises(DuplicateRepresentationError) as caught:
            IdentityMap(IdentityRole.STATE, ((Phase.OPEN, "same"), (Phase.CLOSED, "same")))
        self.assertEqual(caught.exception.context["representation"], "same")

    def test_bounds_are_explicit_and_input_is_consumed_lazily(self):
        with self.assertRaises(IdentityMapLimitError) as caught:
            IdentityMap(IdentityRole.STATE, ((i, i) for i in itertools.count()), max_entries=2)
        self.assertEqual((caught.exception.context["limit"], caught.exception.context["entry"]), (2, 2))
        self.assertIsInstance(caught.exception, LimitError)
        for limit in (0, 4097, 1.5, True):
            with self.assertRaises(IdentityMapLimitError):
                IdentityMap(IdentityRole.STATE, itertools.count(), max_entries=limit)
        self.assertEqual(len(IdentityMap(IdentityRole.STATE, ((i, -i) for i in range(4096)))), 4096)

    def test_malformed_entries_and_roles_fail(self):
        with self.assertRaises(IdentityMapError):
            IdentityMap(IdentityRole.STATE, (([], "list"),))
        with self.assertRaises(IdentityMapError) as caught:
            IdentityMap(IdentityRole.STATE, ((Phase.OPEN,),))
        self.assertEqual(caught.exception.context["entry"], 0)
        with self.assertRaises(IdentityRoleError):
            IdentityMap("state", ())

class MachineIdentityMapsTests(unittest.TestCase):
    def test_each_role_accepts_only_its_own_codec(self):
        states = IdentityMap(IdentityRole.STATE, ((Phase.OPEN, "open"),))
        events = IdentityMap(IdentityRole.EVENT, ((Input.CLOSE, "open"),))
        maps = MachineIdentityMaps(state=states, event=events)
        self.assertIs(maps.decode(IdentityRole.STATE, "open"), Phase.OPEN)
        self.assertIs(maps.decode(IdentityRole.EVENT, "open"), Input.CLOSE)
        self.assertIs(maps.map(IdentityRole.STATE), states)
        with self.assertRaises(UnknownIdentityError):
            maps.encode(IdentityRole.EVENT, Phase.OPEN)
        with self.assertRaises(IdentityRoleError) as caught:
            MachineIdentityMaps(event=states)
        self.assertIs(caught.exception.context["actual"], IdentityRole.STATE)
        with self.assertRaises(IdentityRoleError):
            maps.map(IdentityRole.TRANSITION)
        with self.assertRaises(IdentityRoleError):
            maps.decode("state", "open")

    def test_external_names_decode_into_a_bindable_record(self):
        maps = MachineIdentityMaps(
            state=IdentityMap(IdentityRole.STATE, ((Phase.OPEN, "ex:Open"), (Phase.CLOSED, "ex:Closed"))),
            event=IdentityMap(IdentityRole.EVENT, ((Input.CLOSE, "ex:close"),)),
            transition=IdentityMap(IdentityRole.TRANSITION, (("t1", "ex:t1"),)),
            action=IdentityMap(IdentityRole.ACTION, (("count", "ex:count"),)),
            story=IdentityMap(IdentityRole.STORY, (("count-story", "ex:CountStory"),)),
            implementation=IdentityMap(IdentityRole.IMPLEMENTATION, (("v1", "ex:CountV1"),)))
        document = {"initial": "ex:Open", "states": ["ex:Open", "ex:Closed"],
            "edges": [("ex:t1", "ex:Open", "ex:close", "ex:Closed", "ex:count", "ex:CountStory", "ex:CountV1")]}
        decode = maps.decode
        record = DefinitionRecord(id=object(), initial=decode(IdentityRole.STATE, document["initial"]),
            states=tuple(StateRecord(decode(IdentityRole.STATE, s), terminal=s == "ex:Closed") for s in document["states"]),
            transitions=tuple(TransitionDefinitionRecord(decode(IdentityRole.TRANSITION, t),
                decode(IdentityRole.STATE, a), decode(IdentityRole.EVENT, e), decode(IdentityRole.STATE, b),
                actions=(BehaviorUse(decode(IdentityRole.ACTION, u), BehaviorRef(
                    decode(IdentityRole.STORY, story), decode(IdentityRole.IMPLEMENTATION, version))),))
                for t, a, e, b, u, story, version in document["edges"]))
        reference = BehaviorRef("count-story", "v1")
        behaviors = BehaviorRegistry((Behavior(reference, BehaviorRole.ACTION,
            lambda data, payload, facts: data.append(payload)),))
        with record.bind(behaviors).instantiate(id=object(), data=[]) as machine:
            outcome = machine.dispatch(Event(Input.CLOSE, 5))
            self.assertEqual(maps.encode(IdentityRole.TRANSITION, outcome.record.transition), "ex:t1")
            self.assertEqual(maps.encode(IdentityRole.STATE, machine.state), "ex:Closed")
            self.assertEqual(machine.data, [5])
