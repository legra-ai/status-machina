"""GC-visible native value ownership and synchronous callback invocation."""
from __future__ import annotations
import copy
from contextlib import contextmanager
import inspect
import json
import threading
from typing import NoReturn, TypeVar
from . import _native
from . import errors
from .model import CapabilityStatus

_ERROR_TYPES = {
    "definition": errors.DefinitionError, "callback": errors.CallbackError,
    "invariant": errors.InvariantError, "postcondition": errors.PostconditionError,
    "ambiguous": errors.AmbiguousTransitionError, "action": errors.ActionError,
    "stale_revision": errors.StaleDecisionError, "foreign_definition": errors.ForeignDecisionError,
    "foreign_instance": errors.ForeignDecisionError, "revision_exhausted": errors.RevisionError,
    "inspection": errors.InspectionError,
    "disposed": errors.DisposedError, "busy": errors.BusyError,
}

def translate(error, ids=None) -> NoReturn:
    info = json.loads(str(error))
    code = info["code"]
    if code not in _ERROR_TYPES:
        raise RuntimeError("Unrecognized native failure category") from error
    context = {key: value for key, value in info.items() if key not in ("code", "message") and value is not None}
    for role in ("state", "condition"):
        if role in context and ids is not None:
            context[role] = ids.resolve(role, context[role])
    for key in ("expected_revision", "current_revision"):
        if key in context:
            context[key] = int(context[key])
    exception = _ERROR_TYPES[code]
    if code == "callback" and info.get("phase") == "snapshot":
        exception = errors.SnapshotError
    raise exception(info["message"], **context) from error.__cause__

class Context:
    """One instance arena; no definition-level inventory roots instance payloads."""
    def __init__(self, definition, identity):
        self.definition = definition
        self.ids = definition._compiler.ids
        self.snapshot = definition._snapshot
        self.input_snapshot = definition._input_snapshot
        self.identity = identity
        self.capacity = definition._limits.max_values
        self.values: dict[int, object] = {}
        self.next_key = 0
        self.free_keys: list[int] = []
        self.arena_id: str | None = None
        self.native: _native._Instance | None = None
        self.closed = False
        self._lock = threading.Lock()
        self._sources = None

    def allocate(self, value):
        if len(self.values) == self.capacity:
            raise errors.LimitError("Native value arena is full")
        if self.free_keys:
            key = self.free_keys.pop()
        else:
            if self.next_key == 2**32:
                raise errors.LimitError("Native value key space is exhausted")
            key = self.next_key
            self.next_key += 1
        self.values[key] = value
        return key

    def sweep(self):
        live = set() if self.native is None else set(self.native.live_keys())
        for key in tuple(self.values):
            if key not in live:
                del self.values[key]
                self.free_keys.append(key)

    def resolve(self, key):
        if self.arena_id is None:
            self.arena_id = key["arena"]
        if key["arena"] == self.arena_id:
            return self.values[key["key"]]
        if self._sources is None or key["arena"] not in self._sources:
            raise RuntimeError("Native value belongs to an unbound arena")
        return self._sources[key["arena"]].values[key["key"]]

    @contextmanager
    def operation(self, *, allow_closed=False, sources=None):
        if not self._lock.acquire(blocking=False):
            raise errors.BusyError("An operation is already active on this instance")
        try:
            if self.closed and not allow_closed:
                raise errors.DisposedError("The machine has been disposed")
            self.sweep()
            self._sources = sources
            try:
                yield self
            except _native.NativeFailure as error:
                translate(error, self.ids)
            finally:
                self._sources = None
                self.sweep()
        finally:
            self._lock.release()

    def detach(self):
        self.closed = True
        self.definition = None
        self.snapshot = None
        self.input_snapshot = None
        self.ids = None
        self.identity = None

    def copy_value(self, value, *, input=False):
        result = (self.input_snapshot if input else self.snapshot)(value)
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("Snapshot callbacks must return synchronously, not an awaitable")
        return result

    def invoke(self, encoded):
        request = json.loads(encoded)
        kind = request["kind"]
        values = [self.resolve(key) for key in request["values"]]
        if kind == "snapshot":
            return json.dumps({"kind": "value", "value": self.allocate(self.copy_value(values[0]))})
        entry = self.definition._callbacks[request["callback"]]
        callback = entry.function
        if kind == "invariant":
            args = (self.copy_value(values[0]),)
        elif kind == "capability":
            args = (self.ids.resolve("state", request["state"]), self.copy_value(values[0]),
                self.copy_value(values[1], input=True))
        elif kind in ("predicate", "construct", "action"):
            data = values[0] if kind == "action" else self.copy_value(values[0])
            args = (data, self.copy_value(values[1], input=True), self.copy_value(values[2], input=True))
        else:
            raise RuntimeError("Unsupported native callback request")
        result = callback(*args)
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("Machine callbacks must return synchronously, not an awaitable")
        if kind in ("predicate", "invariant"):
            if type(result) is not bool:
                raise TypeError("Predicates and invariants must return bool")
            response = {"kind": "bool", "value": result}
        elif kind == "capability":
            if not isinstance(result, CapabilityStatus):
                raise TypeError("Capabilities must return CapabilityStatus")
            response = {"kind": "capability", "value": result.value}
        elif kind == "action" and not entry.update:
            if result is not None:
                raise TypeError("An apply action must return None; use update to replace data")
            response = {"kind": "unit"}
        else:
            response = {"kind": "value", "value": self.allocate(result)}
        return json.dumps(response)

_SnapshotT = TypeVar("_SnapshotT")

def DEFAULT_SNAPSHOT(value: _SnapshotT) -> _SnapshotT:
    return copy.deepcopy(value)
