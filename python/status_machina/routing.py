"""Composition values: addresses, routes, registries, deliveries, items and reports."""
from __future__ import annotations
from dataclasses import dataclass, field
from enum import Enum
from itertools import islice
import json
from typing import Any, Generic, Hashable, Iterable, Iterator, Literal, TypeAlias, overload
from typing_extensions import TypeVar
from . import _native
from .errors import PropagationLimitsError, StatusMachinaError
from .machine import Machine
from .model import CapabilityStatus, EffectIntent, Event, Ignored, OutputIntent, Refused, Transitioned

ScopeT = TypeVar("ScopeT", bound=Hashable, default=Hashable, covariant=True)
InstanceT = TypeVar("InstanceT", bound=Hashable, default=Hashable, covariant=True)
KindT = TypeVar("KindT", bound=Hashable, default=Hashable)
PayloadT = TypeVar("PayloadT", default=Any)

# Kernel ceilings: they bound how many native entries are read, never what is accepted.
HARD_MAX_INSTANCES = 512
HARD_MAX_ROUTES = 512

def same(left: object, right: object) -> bool:
    """Type-qualified identity equality: ``1``, ``True`` and IntEnum members never alias."""
    return type(left) is type(right) and left == right

def representable(value: object) -> int | None:
    """A bound the kernel can receive; anything else reaches it as absent or zero."""
    return value if type(value) is int and 0 <= value < 2**64 else None

def read(values: Iterable[object], limit: object, ceiling: int) -> tuple:
    """Read at most one entry beyond the declared bound, so the kernel decides the bound."""
    bound = min(limit, ceiling) if type(limit) is int and limit > 0 else 0
    return tuple(islice(values, bound + 1))

@dataclass(frozen=True, eq=False)
class InstanceAddress(Generic[ScopeT, InstanceT]):
    """A caller-owned address: one machine instance identity within one routing scope.

    Equality and hashing are type-qualified, so scopes and instances of different
    types never alias. The same instance identity in two scopes is two addresses.
    """
    scope: ScopeT
    instance: InstanceT

    def __post_init__(self) -> None:
        try:
            hash(self._key)
        except TypeError as error:
            raise TypeError("Address scopes and instances must be immutable hashable values") from error

    @property
    def _key(self) -> tuple[type, Hashable, type, Hashable]:
        return (type(self.scope), self.scope, type(self.instance), self.instance)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, InstanceAddress):
            return NotImplemented
        return self._key == other._key

    def __hash__(self) -> int:
        return hash(self._key)

@dataclass(frozen=True)
class Route(Generic[KindT]):
    """Deliver ``event`` emitted at ``source`` to ``destination`` as ``input_event``.

    With ``capability``, the route defers unless a snapshot observes it available
    at the destination's current revision.
    """
    source: InstanceAddress
    event: Hashable
    destination: InstanceAddress
    input_event: KindT
    capability: Hashable | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.source, InstanceAddress) or not isinstance(self.destination, InstanceAddress):
            raise TypeError("Route endpoints must be InstanceAddress values")

@dataclass(frozen=True)
class InstanceRegistry:
    """Bounded ``(address, machine)`` registrations; the composition retains no machine."""
    entries: tuple[tuple[InstanceAddress, Machine[Any, Any, Any]], ...]
    max_instances: int = 128

    def __post_init__(self) -> None:
        if type(self.max_instances) is not int:
            raise TypeError("max_instances must be an integer")
        entries = read(self.entries, self.max_instances, HARD_MAX_INSTANCES)
        for entry in entries:
            if (not isinstance(entry, tuple) or len(entry) != 2 or not isinstance(entry[0], InstanceAddress)
                    or not isinstance(entry[1], Machine)):
                raise TypeError("Registry entries must be (InstanceAddress, Machine) pairs")
        object.__setattr__(self, "entries", entries)

@dataclass(frozen=True)
class Delivery(Generic[KindT, PayloadT]):
    """An input addressed to exactly one registered instance."""
    destination: InstanceAddress
    event: Event[KindT, PayloadT]

    def __post_init__(self) -> None:
        if not isinstance(self.destination, InstanceAddress) or not isinstance(self.event, Event):
            raise TypeError("A Delivery needs an InstanceAddress and an Event")

@dataclass(frozen=True)
class PropagationLimits:
    """Explicit work budgets for one propagate call, each within 1..4096."""
    max_steps: int
    max_intents: int

    def __post_init__(self) -> None:
        for name in ("max_steps", "max_intents"):
            if type(getattr(self, name)) is not int:
                raise TypeError(f"{name} must be an integer")
        try:
            _native.propagation_limits(representable(self.max_steps), representable(self.max_intents))
        except _native.NativeFailure as error:
            raise PropagationLimitsError(json.loads(str(error))["message"], reason="invalid_limits",
                max_steps=self.max_steps, max_intents=self.max_intents) from None

class PropagationStatus(Enum):
    """Why propagation returned; only DISPATCH_FAILED carries an error."""
    QUIESCENT = "quiescent"
    STEP_LIMIT = "step_limit"
    INTENT_LIMIT = "intent_limit"
    UNBOUND_DESTINATION = "unbound_destination"
    DISPATCH_FAILED = "dispatch_failed"

@dataclass(frozen=True)
class Routed:
    """A routed event, not yet dispatched; the payload is the original object."""
    source: InstanceAddress
    destination: InstanceAddress
    event: Event
    kind: Literal["routed"] = field(default="routed", init=False)

@dataclass(frozen=True)
class Unmatched:
    """No route was declared; the original intent stays caller-owned."""
    source: InstanceAddress
    intent: OutputIntent
    kind: Literal["unmatched"] = field(default="unmatched", init=False)

@dataclass(frozen=True)
class Deferred:
    """The route's capability was not observed available; the intent stays caller-owned."""
    source: InstanceAddress
    destination: InstanceAddress
    capability: Hashable
    status: CapabilityStatus
    intent: OutputIntent
    kind: Literal["deferred"] = field(default="deferred", init=False)

RoutedItem: TypeAlias = Routed | Unmatched | Deferred

@dataclass(frozen=True)
class RoutedBatch:
    """One ordered item per routed output; nothing has been dispatched."""
    items: tuple[RoutedItem, ...]

    def __iter__(self) -> Iterator[RoutedItem]:
        return iter(self.items)

    def __len__(self) -> int:
        return len(self.items)

    @overload
    def __getitem__(self, index: int) -> RoutedItem: ...
    @overload
    def __getitem__(self, index: slice) -> tuple[RoutedItem, ...]: ...
    def __getitem__(self, index: int | slice) -> RoutedItem | tuple[RoutedItem, ...]:
        return self.items[index]

StepOutcome: TypeAlias = Refused[Any, Any] | Ignored[Any, Any] | Transitioned[Any]

@dataclass(frozen=True)
class PropagationStep:
    """One completed dispatch; its effects and outputs are reported separately."""
    destination: InstanceAddress
    outcome: StepOutcome

@dataclass(frozen=True)
class AddressedEffect:
    """Accepted external work with the instance that requested it; nothing ran."""
    source: InstanceAddress
    intent: EffectIntent

@dataclass(frozen=True)
class PropagationReport:
    """Ordered partial progress of one propagate call.

    Accepted steps are never rolled back by a later failure or budget stop.
    ``pending`` holds the unaccepted input first, then remaining work, and can be
    propagated again; caller inputs are returned as the original Delivery objects.
    """
    status: PropagationStatus
    steps: tuple[PropagationStep, ...]
    effects: tuple[AddressedEffect, ...]
    unrouted: tuple[Unmatched | Deferred, ...]
    pending: tuple[Delivery, ...]
    error: StatusMachinaError | None = None
