"""Lower native specifications to private, role-separated engine indexes."""
from __future__ import annotations
from dataclasses import dataclass
import inspect
from typing import Callable, Hashable, cast
from .errors import DefinitionError, LimitError
from .model import Action, BehaviorRef, BehaviorRegistry, Condition, Limits

UNKNOWN_EVENT = 2**32 - 1

class Identities:
    def __init__(self) -> None:
        self.values: dict[str, list[Hashable]] = {}
        self.indexes: dict[str, dict[tuple[type, Hashable], int]] = {}

    def intern(self, role: str, value: Hashable) -> int:
        key = (type(value), value)
        try:
            hash(key)
        except TypeError as error:
            raise DefinitionError("Identifiers must be immutable hashable native values", role=role) from error
        indexes = self.indexes.setdefault(role, {})
        if key in indexes:
            return indexes[key]
        values = self.values.setdefault(role, [])
        if len(values) == 65536:
            raise LimitError("Identifier inventory exceeds 65536 entries", role=role)
        index = len(values)
        values.append(value)
        indexes[key] = index
        return index

    def find(self, role: str, value: Hashable) -> int:
        return self.indexes.get(role, {}).get((type(value), value), UNKNOWN_EVENT)

    def resolve(self, role: str, value: int) -> Hashable:
        return self.values[role][value]

@dataclass(frozen=True)
class Callback:
    function: Callable[..., object]
    update: bool = False

class Compiler:
    def __init__(self, limits: Limits) -> None:
        self.ids = Identities()
        self.callbacks: list[Callback] = []
        self.limits = limits
        if not 1 <= limits.max_values <= 65536:
            raise LimitError("max_values must be between 1 and 65536")
        if not 1 <= limits.max_callbacks <= 65536:
            raise LimitError("max_callbacks must be between 1 and 65536")

    def reference(self, value: BehaviorRef) -> dict:
        return {"story": self.ids.intern("story", value.story),
                "implementation": self.ids.intern("implementation", value.implementation)}

    def implementation(self, value: Callable | BehaviorRef, *, update: bool = False) -> dict:
        if isinstance(value, BehaviorRef):
            if update:
                raise DefinitionError("Select functional update mode on the registered implementation")
            return {"callback": None, "reference": self.reference(value)}
        if not callable(value):
            raise DefinitionError("A behavior must be a callable or exact BehaviorRef")
        if inspect.iscoroutinefunction(value) or inspect.iscoroutinefunction(getattr(value, "__call__", None)):
            raise DefinitionError("Machine callbacks must be synchronous")
        if len(self.callbacks) == self.limits.max_callbacks:
            raise LimitError("Callback inventory exceeds max_callbacks")
        callback = len(self.callbacks)
        self.callbacks.append(Callback(value, update))
        return {"callback": callback, "reference": None}

    def action(self, action: Action) -> dict:
        if (action.apply is None) == (action.update is None):
            raise DefinitionError("An action needs exactly one apply or update implementation")
        if action.update is not None:
            implementation = self.implementation(action.update, update=True)
        else:
            implementation = self.implementation(cast(Callable | BehaviorRef, action.apply))
        return {"id": self.ids.intern("action", action.id), "implementation": implementation}

    def named(self, value, role: str) -> dict:
        identifier = value.kind if role == "event" else value.id
        return {"id": self.ids.intern(role, identifier), "implementation": self.implementation(value.describe)}

    def condition(self, condition: Condition, depth: int = 1) -> dict:
        if depth > 32:
            raise DefinitionError("Condition nesting exceeds 32")
        forms = (condition.predicate, condition.all_of, condition.any_of, condition.negated)
        if sum(item is not None for item in forms) != 1:
            raise DefinitionError("A condition needs exactly one expression")
        if condition.predicate is not None:
            expression = {"kind": "predicate", "implementation": self.implementation(condition.predicate)}
        elif condition.all_of is not None or condition.any_of is not None:
            children = condition.all_of if condition.all_of is not None else condition.any_of
            expression = {"kind": "all" if condition.all_of is not None else "any",
                          "children": [self.condition(child, depth + 1) for child in cast(tuple[Condition, ...], children)]}
        else:
            expression = {"kind": "not", "child": self.condition(cast(Condition, condition.negated), depth + 1)}
        return {"id": self.ids.intern("condition", condition.id), "expression": expression}

    def state(self, state) -> dict:
        return {"id": self.ids.intern("state", state.id), "terminal": state.terminal,
            "invariant": None if state.invariant is None else self.implementation(state.invariant),
            "entry_actions": [self.action(a) for a in state.entry_actions],
            "exit_actions": [self.action(a) for a in state.exit_actions],
            "entry_effects": [self.named(e, "effect") for e in state.entry_effects],
            "exit_effects": [self.named(e, "effect") for e in state.exit_effects]}

    def transition(self, edge) -> dict:
        return {"id": self.ids.intern("transition", edge.id),
            "source": self.ids.intern("state", edge.source), "target": self.ids.intern("state", edge.target),
            "event": self.ids.intern("event", edge.event), "internal": edge.internal,
            "guard": None if edge.guard is None else self.condition(edge.guard),
            "postconditions": [self.condition(c) for c in edge.postconditions],
            "actions": [self.action(a) for a in edge.actions],
            "effects": [self.named(e, "effect") for e in edge.effects],
            "outputs": [self.named(e, "event") for e in edge.outputs]}

    def disposition(self, value) -> dict:
        return {"id": self.ids.intern("disposition", value.id), "source": self.ids.intern("state", value.source),
            "event": self.ids.intern("event", value.event), "condition": self.condition(value.condition),
            "reason": value.reason}

    def behaviors(self, registry: BehaviorRegistry) -> list[dict]:
        result = []
        for behavior in registry.implementations:
            implementation = self.implementation(behavior.callback, update=behavior.update)
            result.append({"reference": self.reference(behavior.reference),
                "role": behavior.role.value, "callback": implementation["callback"]})
        return result
