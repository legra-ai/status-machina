"""Static contracts for portable records and role-aware identity codecs."""
from enum import Enum, auto
from typing import Hashable
from typing_extensions import assert_type
from status_machina import (
    BehaviorRef, BehaviorRegistry, BehaviorUse, ConditionRecord, Definition, DefinitionRecord,
    IdentityMap, IdentityRole, MachineIdentityMaps, NotRecord, PredicateRecord, StateRecord,
    TransitionDefinitionRecord,
)

class Phase(Enum):
    OPEN = auto()
    DONE = auto()

class Input(Enum):
    FINISH = auto()

class Wrong(Enum):
    OTHER = auto()

reference = BehaviorRef(story=object(), implementation=object())
guard: ConditionRecord = NotRecord(id=object(), operand=PredicateRecord(BehaviorUse(object(), reference)))
record = DefinitionRecord[Phase, Input](
    id=object(), initial=Phase.OPEN,
    states=(StateRecord(Phase.OPEN), StateRecord(Phase.DONE, terminal=True)),
    transitions=(TransitionDefinitionRecord(object(), Phase.OPEN, Input.FINISH, Phase.DONE, guard=guard),),
)
assert_type(record.initial, Phase)
definition = record.bind(BehaviorRegistry(), snapshot=lambda value: value + 1)
assert_type(definition, Definition[Phase, Input, int])
assert_type(record.bind(BehaviorRegistry()).initial, Phase)
DefinitionRecord[Phase, Input](id=object(), initial=Wrong.OTHER, states=(), transitions=())  # type: ignore[arg-type]
StateRecord(Phase.OPEN, invariant=lambda data: True)  # type: ignore[arg-type]
BehaviorUse(object(), lambda: None)  # type: ignore[arg-type]
record.bind(())  # type: ignore[arg-type]

states = IdentityMap[Phase, str](IdentityRole.STATE, ((Phase.OPEN, "ex:Open"),))
assert_type(states.encode(Phase.OPEN), str)
assert_type(states.decode("ex:Open"), Phase)
states.encode(Wrong.OTHER)  # type: ignore[arg-type]
states.decode(1)  # type: ignore[arg-type]
IdentityMap[Phase, str]("state", ())  # type: ignore[arg-type]
maps = MachineIdentityMaps[Phase, Input, str](state=states)
assert maps.state is not None
assert_type(maps.state.decode("ex:Open"), Phase)
assert_type(maps.decode(IdentityRole.STATE, "ex:Open"), Hashable)
assert_type(maps.encode(IdentityRole.STATE, Phase.OPEN), str)
MachineIdentityMaps[Phase, Input, str](event=states)  # type: ignore[arg-type]
