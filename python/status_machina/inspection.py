"""Bounded structural reports and explicit Mermaid labels, independent of identity."""
from __future__ import annotations
from collections.abc import Mapping
from dataclasses import dataclass, field
import json
from typing import TYPE_CHECKING, Generic, Hashable
from typing_extensions import TypeVar
from .errors import InspectionError

if TYPE_CHECKING:
    from ._compile import Identities

StateT = TypeVar('StateT', bound=Hashable, default=Hashable)
KindT = TypeVar('KindT', bound=Hashable, default=Hashable)

@dataclass(frozen=True)
class InspectionLimits:
    """At most 1000000 graph entries and 1 MiB Mermaid UTF-8 output."""
    max_entries: int = 4096
    max_output_bytes: int = 1_048_576
    def __post_init__(self) -> None:
        for name, maximum in (('max_entries', 1_000_000), ('max_output_bytes', 1_048_576)):
            value = getattr(self, name)
            if type(value) is not int or not 1 <= value <= maximum:
                raise InspectionError(f'{name} must be an integer between 1 and {maximum}')
    def check_entries(self, inventory: dict[str, list[int]]) -> None:
        if sum(len(inventory[role]) for role in ('states', 'transitions', 'dispositions')) > self.max_entries:
            raise InspectionError('Graph inventory exceeds max_entries', limit=self.max_entries)

@dataclass(frozen=True)
class StateCoverage(Generic[StateT]):
    """Declared edges/dispositions at one state; not proof of execution coverage."""
    id: StateT
    initial: bool
    terminal: bool
    incoming: int
    outgoing: int
    dispositions: int

@dataclass(frozen=True)
class StructuralCoverage(Generic[StateT]):
    """Immutable graph inventory in definition insertion order."""
    states: tuple[StateCoverage[StateT], ...]
    transitions: tuple[Hashable, ...]
    dispositions: tuple[Hashable, ...]

@dataclass(frozen=True)
class DiagramLabels(Generic[StateT, KindT]):
    """Explicit text for each rendered identity; equal labels do not merge nodes."""
    states: Mapping[StateT, str] = field(default_factory=dict)
    transitions: Mapping[Hashable, str] = field(default_factory=dict)
    events: Mapping[KindT, str] = field(default_factory=dict)
    conditions: Mapping[Hashable, str] = field(default_factory=dict)
    dispositions: Mapping[Hashable, str] = field(default_factory=dict)

    def _encode(self, ids: Identities, inventory: dict[str, list[int]], limits: InspectionLimits) -> str:
        limits.check_entries(inventory)
        remaining = limits.max_output_bytes
        result = {}
        for name, role in (('states', 'state'), ('transitions', 'transition'), ('events', 'event'),
                ('conditions', 'condition'), ('dispositions', 'disposition')):
            mapping = getattr(self, name)
            if not isinstance(mapping, Mapping):
                raise InspectionError('Diagram labels must be mappings', role=role)
            labels = []
            # bounded: the native inventory belongs to the validated, bounded definition.
            for index in inventory[name]:
                identifier = ids.resolve(role, index)
                if identifier not in mapping:
                    raise InspectionError('Missing diagram label', role=role, identifier=identifier)
                text = mapping[identifier]
                if not isinstance(text, str):
                    raise InspectionError('Diagram labels must be strings', role=role, identifier=identifier)
                if len(text) > remaining:
                    raise InspectionError('Label UTF-8 exceeds max_output_bytes', limit=limits.max_output_bytes)
                for character in text:
                    code = ord(character)
                    if 0xD800 <= code <= 0xDFFF:
                        raise InspectionError('Diagram labels must contain valid Unicode', role=role, identifier=identifier)
                    remaining -= 1 if code < 0x80 else 2 if code < 0x800 else 3 if code < 0x10000 else 4
                    if remaining < 0:
                        raise InspectionError('Label UTF-8 exceeds max_output_bytes', limit=limits.max_output_bytes)
                labels.append({'id': index, 'text': text})
            result[name] = labels
        return json.dumps(result, ensure_ascii=False, separators=(',', ':'))
