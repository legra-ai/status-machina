"""Role-aware identity codecs between native identities and external representations.

An ``IdentityMap`` is the Rust ``IdentityMap<T, R>`` for one ``IdentityRole``:
bounded (1 to 4096 entries), one-to-one and explicit. The kernel validates the
entries; Python only interns values with type-qualified keys, so ``1``, ``True``
and members of different ``IntEnum`` classes never alias. Encoding never falls
back to ``str``/``repr``, and decoding never substitutes a default. Runtime
matching never consults a codec: it is for adapters that read or write names,
IRIs or other caller-owned representations around the native API.
"""
from __future__ import annotations

import json
from collections.abc import Hashable, Iterable, Iterator
from dataclasses import dataclass, fields
from enum import Enum
from typing import Generic, NoReturn

from typing_extensions import TypeVar

from . import _native, errors

IdT = TypeVar("IdT", bound=Hashable, default=Hashable)
ReprT = TypeVar("ReprT", bound=Hashable, default=Hashable)
StateT = TypeVar("StateT", bound=Hashable, default=Hashable)
KindT = TypeVar("KindT", bound=Hashable, default=Hashable)

_MAX_ENTRIES = 4096

class IdentityRole(Enum):
    """The identity roles of Rust ``MachineIds``; each keeps its own inventory."""
    DEFINITION = "definition"
    INSTANCE = "instance"
    STATE = "state"
    EVENT = "event"
    TRANSITION = "transition"
    ACTION = "action"
    CONDITION = "condition"
    EFFECT = "effect"
    DISPOSITION = "disposition"
    CAPABILITY = "capability"
    OPERATION = "operation"
    SCENARIO = "scenario"
    ASSERTION = "assertion"
    SCOPE = "scope"
    STORY = "story"
    """Reusable behavior (EKGF Story) identity: Rust ``MachineIds::Behavior``."""
    IMPLEMENTATION = "implementation"

_FAILURES: dict[str, type[errors.IdentityMapError]] = {
    "invalid_limit": errors.IdentityMapLimitError, "limit_exceeded": errors.IdentityMapLimitError,
    "duplicate_identity": errors.DuplicateIdentityError,
    "duplicate_representation": errors.DuplicateRepresentationError,
    "unknown_identity": errors.UnknownIdentityError,
    "unknown_representation": errors.UnknownRepresentationError,
}

def _key(value: object, role: IdentityRole, label: str) -> tuple[type, object]:
    key = (type(value), value)
    try:
        hash(key)
    except TypeError as error:
        raise errors.IdentityMapError(f"{label} must be an immutable hashable native value",
            role=role) from error
    return key

class IdentityMap(Generic[IdT, ReprT]):
    """Bounded one-to-one codec for a single identity role, validated by the kernel."""
    __slots__ = ("_entries", "_identities", "_max_entries", "_native", "_representations", "_role")

    def __init__(self, role: IdentityRole, entries: Iterable[tuple[IdT, ReprT]], *,
            max_entries: int = _MAX_ENTRIES) -> None:
        if not isinstance(role, IdentityRole):
            raise errors.IdentityRoleError("IdentityMap.role must be an IdentityRole member")
        if type(max_entries) is not int:
            raise errors.IdentityMapLimitError("max_entries must be an integer between 1 and 4096", role=role)
        self._role = role
        self._max_entries = max_entries
        # Consume at most one entry past the bound; the kernel decides validity.
        budget = max_entries + 1 if 1 <= max_entries <= _MAX_ENTRIES else 0
        accepted: list[tuple[IdT, ReprT]] = []
        identities: dict[tuple[type, object], int] = {}
        representations: dict[tuple[type, object], int] = {}
        pairs: list[tuple[int, int]] = []
        for entry in entries:
            if len(accepted) == budget:
                break
            try:
                identity, representation = entry
            except (TypeError, ValueError) as error:
                raise errors.IdentityMapError("IdentityMap entries must be (identity, representation) pairs",
                    role=role, entry=len(accepted)) from error
            identity_index = identities.setdefault(_key(identity, role, "Identity"), len(accepted))
            representation_index = representations.setdefault(
                _key(representation, role, "Representation"), len(accepted))
            accepted.append((identity, representation))
            pairs.append((identity_index, representation_index))
        try:
            self._native = _native._IdentityMap(pairs, max_entries)
        except _native.NativeFailure as error:
            self._raise(error, accepted)
        self._entries = tuple(accepted)
        self._identities = identities
        self._representations = representations

    @property
    def role(self) -> IdentityRole:
        return self._role

    @property
    def max_entries(self) -> int:
        return self._max_entries

    def __len__(self) -> int:
        return len(self._entries)

    def __iter__(self) -> Iterator[tuple[IdT, ReprT]]:
        """Registered ``(identity, representation)`` pairs in declaration order."""
        return iter(self._entries)

    def encode(self, identity: IdT) -> ReprT:
        """Return the registered representation; never derived from ``str``/``repr``."""
        index = self._identities.get(_key(identity, self._role, "Identity"))
        if index is None:
            raise errors.UnknownIdentityError("unknown mapped identity", role=self._role, identity=identity)
        try:
            return self._entries[self._native.encode(index)][1]
        except _native.NativeFailure as error:
            self._raise(error, self._entries)

    def decode(self, representation: ReprT) -> IdT:
        """Return the registered identity; unknown representations are errors."""
        index = self._representations.get(_key(representation, self._role, "Representation"))
        if index is None:
            raise errors.UnknownRepresentationError("unknown identity representation",
                role=self._role, representation=representation)
        try:
            return self._entries[self._native.decode(index)][0]
        except _native.NativeFailure as error:
            self._raise(error, self._entries)

    def _raise(self, error: Exception, entries: list[tuple[IdT, ReprT]] | tuple[tuple[IdT, ReprT], ...]) -> NoReturn:
        info = json.loads(str(error))
        exception = _FAILURES.get(info.get("reason", ""))
        if info.get("code") != "identity_map" or exception is None:
            raise RuntimeError("Unrecognized native identity map failure") from error
        context: dict[str, object] = {"role": self._role}
        if info["limit"] is not None:
            context["limit"] = info["limit"]
        position = info["entry"]
        if position is not None:
            context["entry"] = position
            if exception is errors.DuplicateIdentityError:
                context["identity"] = entries[position][0]
            elif exception is errors.DuplicateRepresentationError:
                context["representation"] = entries[position][1]
        raise exception(info["message"], **context) from None

    def __repr__(self) -> str:
        return f"IdentityMap(role={self._role}, entries={len(self._entries)})"

@dataclass(frozen=True)
class MachineIdentityMaps(Generic[StateT, KindT, ReprT]):
    """At most one codec per ``IdentityRole``; each field accepts only its own role."""
    definition: IdentityMap[Hashable, ReprT] | None = None
    instance: IdentityMap[Hashable, ReprT] | None = None
    state: IdentityMap[StateT, ReprT] | None = None
    event: IdentityMap[KindT, ReprT] | None = None
    transition: IdentityMap[Hashable, ReprT] | None = None
    action: IdentityMap[Hashable, ReprT] | None = None
    condition: IdentityMap[Hashable, ReprT] | None = None
    effect: IdentityMap[Hashable, ReprT] | None = None
    disposition: IdentityMap[Hashable, ReprT] | None = None
    capability: IdentityMap[Hashable, ReprT] | None = None
    operation: IdentityMap[Hashable, ReprT] | None = None
    scenario: IdentityMap[Hashable, ReprT] | None = None
    assertion: IdentityMap[Hashable, ReprT] | None = None
    scope: IdentityMap[Hashable, ReprT] | None = None
    story: IdentityMap[Hashable, ReprT] | None = None
    implementation: IdentityMap[Hashable, ReprT] | None = None

    def __post_init__(self) -> None:
        for item in fields(self):
            value = getattr(self, item.name)
            role = IdentityRole(item.name)
            if value is None:
                continue
            if not isinstance(value, IdentityMap):
                raise errors.IdentityRoleError(f"{item.name} must be an IdentityMap", role=role)
            if value.role is not role:
                raise errors.IdentityRoleError(f"{value.role} codec supplied for the {role} role",
                    role=role, actual=value.role)

    def map(self, role: IdentityRole) -> IdentityMap[Hashable, ReprT]:
        """Return the codec for ``role``; an unconfigured role is an error."""
        if not isinstance(role, IdentityRole):
            raise errors.IdentityRoleError("role must be an IdentityRole member")
        value: IdentityMap[Hashable, ReprT] | None = getattr(self, role.value)
        if value is None:
            raise errors.IdentityRoleError(f"no codec is configured for the {role} role", role=role)
        return value

    def encode(self, role: IdentityRole, identity: Hashable) -> ReprT:
        return self.map(role).encode(identity)

    def decode(self, role: IdentityRole, representation: ReprT) -> Hashable:
        return self.map(role).decode(representation)
