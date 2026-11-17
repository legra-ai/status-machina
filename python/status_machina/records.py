"""Portable, data-only definition records bound to explicit Story implementations.

A record holds native identities and exact ``BehaviorRef`` selections only: no
callables, no module paths and no string codec. ``DefinitionRecord.bind``
resolves every use site through the caller's ``BehaviorRegistry`` with the Rust
kernel's ``DefinitionRecord::bind``; nothing is imported or loaded implicitly.
"""
from __future__ import annotations

from collections.abc import Callable, Hashable
from dataclasses import dataclass, field
from typing import Any, Generic, Literal, TypeAlias

from typing_extensions import TypeVar

from ._bounded import bounded
from ._context import DEFAULT_SNAPSHOT
from .errors import ConditionRecordError, LimitError, RecordError
from .machine import Definition
from .model import (
    Action,
    BehaviorRef,
    BehaviorRegistry,
    Condition,
    Disposition,
    Effect,
    OutputEvent,
    State,
    Transition,
)

StateT = TypeVar("StateT", bound=Hashable)
KindT = TypeVar("KindT", bound=Hashable)
IdT = TypeVar("IdT", bound=Hashable, default=Hashable)
DataT = TypeVar("DataT", default=Any)

_MAX_CONDITION_DEPTH = 32
_PHASE_LIMIT = 16

def _identity(value: object, label: str) -> None:
    try:
        hash((type(value), value))
    except TypeError as error:
        raise RecordError(f"{label} must be an immutable hashable native identity") from error

def _reference(value: object, label: str) -> BehaviorRef:
    if not isinstance(value, BehaviorRef):
        raise RecordError(f"{label} must be an exact BehaviorRef; records never carry callables")
    return value

def _uses(values: object, label: str) -> tuple[BehaviorUse, ...]:
    result = bounded(values, _PHASE_LIMIT, label)
    for value in result:
        if not isinstance(value, BehaviorUse):
            raise RecordError(f"{label} must contain BehaviorUse values")
    return result

def _conditions(values: object, label: str) -> tuple[ConditionRecord, ...]:
    result = bounded(values, _PHASE_LIMIT, label)
    for value in result:
        _condition(value, label)
    return result

def _condition(value: object, label: str) -> ConditionRecord:
    if not isinstance(value, (PredicateRecord, AllRecord, AnyRecord, NotRecord)):
        raise RecordError(f"{label} must be a condition record")
    return value

@dataclass(frozen=True)
class BehaviorUse(Generic[IdT]):
    """One graph use site (``id``) selecting an exact Story implementation."""
    id: IdT
    reference: BehaviorRef
    def __post_init__(self) -> None:
        _identity(self.id, "BehaviorUse.id")
        _reference(self.reference, "BehaviorUse.reference")

@dataclass(frozen=True)
class PredicateRecord:
    """A registered condition implementation; ``use.id`` is the condition identity."""
    use: BehaviorUse
    kind: Literal["predicate"] = field(default="predicate", init=False)
    def __post_init__(self) -> None:
        if not isinstance(self.use, BehaviorUse):
            raise RecordError("PredicateRecord.use must be a BehaviorUse")
    @property
    def id(self) -> Hashable:
        return self.use.id
    def _specification(self, depth: int) -> Condition:
        return Condition(self.use.id, predicate=self.use.reference)

@dataclass(frozen=True)
class AllRecord:
    """Every operand must hold, evaluated in declared order (at most 16)."""
    id: Hashable
    operands: tuple[ConditionRecord, ...]
    kind: Literal["all"] = field(default="all", init=False)
    def __post_init__(self) -> None:
        object.__setattr__(self, "operands", _conditions(self.operands, "all-record operands"))
    def _specification(self, depth: int) -> Condition:
        return Condition(self.id, all_of=tuple(_lower(item, depth + 1) for item in self.operands))

@dataclass(frozen=True)
class AnyRecord:
    """At least one operand must hold, evaluated in declared order (at most 16)."""
    id: Hashable
    operands: tuple[ConditionRecord, ...]
    kind: Literal["any"] = field(default="any", init=False)
    def __post_init__(self) -> None:
        object.__setattr__(self, "operands", _conditions(self.operands, "any-record operands"))
    def _specification(self, depth: int) -> Condition:
        return Condition(self.id, any_of=tuple(_lower(item, depth + 1) for item in self.operands))

@dataclass(frozen=True)
class NotRecord:
    """Negate one expression."""
    id: Hashable
    operand: ConditionRecord
    kind: Literal["not"] = field(default="not", init=False)
    def __post_init__(self) -> None:
        _condition(self.operand, "NotRecord.operand")
    def _specification(self, depth: int) -> Condition:
        return Condition(self.id, negated=_lower(self.operand, depth + 1))

ConditionRecord: TypeAlias = PredicateRecord | AllRecord | AnyRecord | NotRecord
"""A borrowed condition expression of references, discriminated by ``kind``."""

def _lower(value: ConditionRecord, depth: int = 1) -> Condition:
    # Bound native recursion with the kernel's own depth rule and identity.
    if depth > _MAX_CONDITION_DEPTH:
        raise ConditionRecordError("condition nesting exceeds 32 levels", condition=value.id)
    return value._specification(depth)

@dataclass(frozen=True)
class StateRecord(Generic[StateT]):
    """A state declaration; each behavior phase holds at most 16 uses."""
    id: StateT
    terminal: bool = False
    invariant: BehaviorRef | None = None
    entry_actions: tuple[BehaviorUse, ...] = ()
    exit_actions: tuple[BehaviorUse, ...] = ()
    entry_effects: tuple[BehaviorUse, ...] = ()
    exit_effects: tuple[BehaviorUse, ...] = ()
    def __post_init__(self) -> None:
        if type(self.terminal) is not bool:
            raise RecordError("StateRecord.terminal must be a bool")
        if self.invariant is not None:
            _reference(self.invariant, "StateRecord.invariant")
        for name in ("entry_actions", "exit_actions", "entry_effects", "exit_effects"):
            object.__setattr__(self, name, _uses(getattr(self, name), name))
    def _specification(self) -> State[StateT]:
        return State(self.id, terminal=self.terminal, invariant=self.invariant,
            entry_actions=tuple(Action(u.id, apply=u.reference) for u in self.entry_actions),
            exit_actions=tuple(Action(u.id, apply=u.reference) for u in self.exit_actions),
            entry_effects=tuple(Effect(u.id, u.reference) for u in self.entry_effects),
            exit_effects=tuple(Effect(u.id, u.reference) for u in self.exit_effects))

@dataclass(frozen=True)
class TransitionDefinitionRecord(Generic[StateT, KindT]):
    """A permitted edge; ``outputs`` use the emitted event kind as their identity."""
    id: Hashable
    source: StateT
    event: KindT
    target: StateT
    internal: bool = False
    guard: ConditionRecord | None = None
    postconditions: tuple[ConditionRecord, ...] = ()
    actions: tuple[BehaviorUse, ...] = ()
    effects: tuple[BehaviorUse, ...] = ()
    outputs: tuple[BehaviorUse, ...] = ()
    def __post_init__(self) -> None:
        if type(self.internal) is not bool:
            raise RecordError("TransitionDefinitionRecord.internal must be a bool")
        if self.guard is not None:
            _condition(self.guard, "TransitionDefinitionRecord.guard")
        object.__setattr__(self, "postconditions", _conditions(self.postconditions, "postconditions"))
        for name in ("actions", "effects", "outputs"):
            object.__setattr__(self, name, _uses(getattr(self, name), name))
    def _specification(self) -> Transition[StateT, KindT]:
        return Transition(self.id, self.source, self.event, self.target, internal=self.internal,
            guard=None if self.guard is None else _lower(self.guard),
            postconditions=tuple(_lower(c) for c in self.postconditions),
            actions=tuple(Action(u.id, apply=u.reference) for u in self.actions),
            effects=tuple(Effect(u.id, u.reference) for u in self.effects),
            outputs=tuple(OutputEvent(u.id, u.reference) for u in self.outputs))

@dataclass(frozen=True)
class DispositionRecord(Generic[StateT, KindT]):
    """An explicitly harmless event; ``reason`` is 1 to 256 UTF-8 bytes."""
    id: Hashable
    source: StateT
    event: KindT
    condition: ConditionRecord
    reason: str
    def __post_init__(self) -> None:
        _condition(self.condition, "DispositionRecord.condition")
        if not isinstance(self.reason, str):
            raise RecordError("DispositionRecord.reason must be a str")
    def _specification(self) -> Disposition[StateT, KindT]:
        return Disposition(self.id, self.source, self.event, _lower(self.condition), self.reason)

@dataclass(frozen=True)
class DefinitionRecord(Generic[StateT, KindT]):
    """A data-only machine graph in behavior declaration order, independent of format."""
    id: Hashable
    initial: StateT
    states: tuple[StateRecord[StateT], ...]
    transitions: tuple[TransitionDefinitionRecord[StateT, KindT], ...]
    dispositions: tuple[DispositionRecord[StateT, KindT], ...] = ()
    max_states: int = 128
    max_transitions: int = 512
    def __post_init__(self) -> None:
        for name in ("max_states", "max_transitions"):
            value = getattr(self, name)
            if type(value) is not int or not 1 <= value <= 65536:
                raise LimitError(f"{name} must be an integer between 1 and 65536")
        _identity(self.id, "DefinitionRecord.id")
        for name, limit, kind in (("states", self.max_states, StateRecord),
                ("transitions", self.max_transitions, TransitionDefinitionRecord),
                ("dispositions", 128, DispositionRecord)):
            values = bounded(getattr(self, name), limit, f"{name} record inventory")
            for value in values:
                if not isinstance(value, kind):
                    raise RecordError(f"DefinitionRecord.{name} must contain {kind.__name__} values")
            object.__setattr__(self, name, values)

    def bind(self, behaviors: BehaviorRegistry, *, snapshot: Callable[[DataT], DataT] = DEFAULT_SNAPSHOT,
            input_snapshot: Callable[[Any], Any] = DEFAULT_SNAPSHOT, max_values: int = 8192) -> Definition[StateT, KindT, DataT]:
        """Resolve every use site in ``behaviors`` and validate through the kernel builders.

        Raises ``BindingError`` subclasses for unknown Stories/implementations or
        a mismatched role, ``ConditionRecordError``/``TransitionRecordError``/
        ``DispositionRecordError`` for invalid records, and ``RecordError`` for
        graph rules. No callback runs during binding.
        """
        return Definition._from_record(self, behaviors, snapshot=snapshot,
            input_snapshot=input_snapshot, max_values=max_values)

    def _specifications(self) -> tuple[tuple[State[StateT], ...], tuple[Transition[StateT, KindT], ...], tuple[Disposition, ...]]:
        return (tuple(s._specification() for s in self.states),
            tuple(t._specification() for t in self.transitions),
            tuple(d._specification() for d in self.dispositions))
