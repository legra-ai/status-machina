"""Python specifications and values; enum members and application classes stay native."""
from __future__ import annotations
from dataclasses import dataclass, field
from enum import Enum
from typing import Callable, Generic, Hashable, Literal, TypeVar

StateT = TypeVar("StateT", bound=Hashable)
KindT = TypeVar("KindT", bound=Hashable)
DataT = TypeVar("DataT")
PayloadT = TypeVar("PayloadT")
FactsT = TypeVar("FactsT")
IdT = TypeVar("IdT", bound=Hashable)

@dataclass(frozen=True)
class BehaviorRef:
    """Exact Story and implementation identities, with independently typed values."""
    story: Hashable
    implementation: Hashable

class BehaviorRole(Enum):
    ACTION = "action"
    CONDITION = "condition"
    INVARIANT = "invariant"
    EFFECT = "effect"
    OUTPUT = "output"
    CAPABILITY = "capability"
    ASSERTION = "assertion"

@dataclass(frozen=True)
class Behavior:
    reference: BehaviorRef
    role: BehaviorRole
    callback: Callable[..., object]
    update: bool = False

@dataclass(frozen=True)
class BehaviorRegistry:
    implementations: tuple[Behavior, ...] = ()
    max_bindings: int = 128
    def __post_init__(self) -> None:
        object.__setattr__(self, "implementations", tuple(self.implementations))

@dataclass(frozen=True)
class Action(Generic[DataT, PayloadT, FactsT]):
    """Mutate the candidate with ``apply`` or replace it with ``update``."""
    id: Hashable
    apply: Callable[[DataT, PayloadT, FactsT], None] | BehaviorRef | None = None
    update: Callable[[DataT, PayloadT, FactsT], DataT] | None = None

@dataclass(frozen=True)
class Condition(Generic[DataT, PayloadT, FactsT]):
    id: Hashable
    predicate: Callable[[DataT, PayloadT, FactsT], bool] | BehaviorRef | None = None
    all_of: tuple[Condition[DataT, PayloadT, FactsT], ...] | None = None
    any_of: tuple[Condition[DataT, PayloadT, FactsT], ...] | None = None
    negated: Condition[DataT, PayloadT, FactsT] | None = None
    def __post_init__(self) -> None:
        if self.all_of is not None:
            object.__setattr__(self, "all_of", tuple(self.all_of))
        if self.any_of is not None:
            object.__setattr__(self, "any_of", tuple(self.any_of))

@dataclass(frozen=True)
class Effect(Generic[PayloadT]):
    id: Hashable
    describe: Callable[..., PayloadT] | BehaviorRef

@dataclass(frozen=True)
class OutputEvent(Generic[KindT, PayloadT]):
    kind: KindT
    describe: Callable[..., PayloadT] | BehaviorRef

@dataclass(frozen=True)
class State(Generic[StateT]):
    id: StateT
    terminal: bool = False
    invariant: Callable[..., bool] | BehaviorRef | None = None
    entry_actions: tuple[Action, ...] = ()
    exit_actions: tuple[Action, ...] = ()
    entry_effects: tuple[Effect, ...] = ()
    exit_effects: tuple[Effect, ...] = ()
    def __post_init__(self) -> None:
        for name in ("entry_actions", "exit_actions", "entry_effects", "exit_effects"):
            object.__setattr__(self, name, tuple(getattr(self, name)))

@dataclass(frozen=True)
class Transition(Generic[StateT, KindT]):
    id: Hashable
    source: StateT
    event: KindT
    target: StateT
    internal: bool = False
    guard: Condition | None = None
    postconditions: tuple[Condition, ...] = ()
    actions: tuple[Action, ...] = ()
    effects: tuple[Effect, ...] = ()
    outputs: tuple[OutputEvent, ...] = ()
    def __post_init__(self) -> None:
        for name in ("postconditions", "actions", "effects", "outputs"):
            object.__setattr__(self, name, tuple(getattr(self, name)))

@dataclass(frozen=True)
class Disposition(Generic[StateT, KindT]):
    id: Hashable
    source: StateT
    event: KindT
    condition: Condition
    reason: str

@dataclass(frozen=True)
class Limits:
    max_states: int = 128
    max_transitions: int = 512
    max_values: int = 8192
    max_callbacks: int = 4096

@dataclass(frozen=True)
class Event(Generic[KindT, PayloadT]):
    kind: KindT
    payload: PayloadT

@dataclass(frozen=True)
class EffectIntent(Generic[IdT, PayloadT]):
    id: IdT
    payload: PayloadT

@dataclass(frozen=True)
class OutputIntent(Generic[KindT, PayloadT]):
    kind: KindT
    payload: PayloadT

@dataclass(frozen=True)
class TransitionRecord(Generic[StateT]):
    transition: Hashable
    source: StateT
    target: StateT
    previous_revision: int
    revision: int

@dataclass(frozen=True)
class Refused(Generic[StateT, KindT]):
    state: StateT
    event: KindT
    kind: Literal["refused"] = field(default="refused", init=False)

@dataclass(frozen=True)
class Ignored(Generic[StateT, KindT]):
    disposition: Hashable
    state: StateT
    event: KindT
    reason: str
    kind: Literal["ignored"] = field(default="ignored", init=False)

@dataclass(frozen=True)
class Transitioned(Generic[StateT]):
    record: TransitionRecord[StateT]
    effects: tuple[EffectIntent, ...]
    outputs: tuple[OutputIntent, ...]
    kind: Literal["transitioned"] = field(default="transitioned", init=False)
