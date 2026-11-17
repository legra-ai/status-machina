"""Retry policies and correlated waits, decided by the kernel's ``RetryPolicy`` and ``RetryWaits``.

Durations are ``datetime.timedelta`` values and instants are timezone-aware
``datetime`` values. The caller supplies every count, elapsed duration, deadline
and current time: nothing here reads a clock, starts a timer or schedules a wake.
A ``RetryWaits`` value is ordinary machine data. Schedule and consume waits from
candidate actions, so a refused transition leaves the accepted waits unchanged;
``copy.deepcopy`` (the default snapshot) copies it with its payloads, keeping the
instance and operation identities themselves.
"""
from __future__ import annotations

import copy
import json
from collections.abc import Hashable
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from typing import Any, Generic, Literal, NoReturn, TypeAlias

from typing_extensions import Self, TypeVar

from . import _native, errors

InstanceT = TypeVar("InstanceT", bound=Hashable, default=Hashable)
OperationT = TypeVar("OperationT", bound=Hashable, default=Hashable)
PayloadT = TypeVar("PayloadT", default=Any)

_EPOCH = datetime(1970, 1, 1, tzinfo=timezone.utc)
_FAILURES: dict[tuple[str, str], type[errors.RetryError]] = {
    ("policy", "zero_delay"): errors.ZeroDelayError,
    ("policy", "too_few_attempts"): errors.TooFewAttemptsError,
    ("policy", "cap_below_initial"): errors.CapBelowInitialError,
    ("waits", "invalid_capacity"): errors.InvalidWaitCapacityError,
    ("waits", "capacity_reached"): errors.WaitCapacityReachedError,
    ("waits", "generation_exhausted"): errors.WaitGenerationExhaustedError,
    ("range", "out_of_range"): errors.RetryRangeError,
}

def _raise(error: Exception, values: dict[str, object]) -> NoReturn:
    info = json.loads(str(error))
    exception = _FAILURES.get((info.get("family"), info.get("reason")))
    if info.get("code") != "retry" or exception is None:
        raise RuntimeError("Unrecognized native retry failure") from error
    context: dict[str, object] = {"reason": info["reason"]}
    if info["field"] is not None:
        context["field"] = info["field"]
        if info["field"] in values:
            context["value"] = values[info["field"]]
    if info["limit"] is not None:
        context["limit"] = info["limit"]
    raise exception(info["message"], **context) from None

def _duration(value: object, name: str) -> dict[str, object]:
    if not isinstance(value, timedelta):
        raise TypeError(f"{name} must be a datetime.timedelta")
    # Negative timedeltas reach the bridge as negative seconds, a range failure.
    return {"seconds": str(value.days * 86400 + value.seconds), "nanoseconds": value.microseconds * 1000}

def _from_duration(value: dict[str, Any]) -> timedelta:
    # Every delay derives from timedelta inputs, so it is a whole number of microseconds.
    return timedelta(seconds=int(value["seconds"]), microseconds=value["nanoseconds"] // 1000)

def _instant(value: object, name: str) -> dict[str, object]:
    if not isinstance(value, datetime):
        raise TypeError(f"{name} must be a datetime.datetime")
    if value.tzinfo is None or value.utcoffset() is None:
        raise ValueError(f"{name} must be timezone-aware; a naive datetime names no instant")
    since = value - _EPOCH
    return {"seconds": str(since.days * 86400 + since.seconds), "nanoseconds": since.microseconds * 1000}

def _from_instant(value: dict[str, Any]) -> datetime:
    return _EPOCH + timedelta(seconds=int(value["seconds"]), microseconds=value["nanoseconds"] // 1000)

def _same(left: object, right: object) -> bool:
    """Type-qualified identity equality: ``1``, ``True`` and IntEnum members never alias."""
    return type(left) is type(right) and left == right

def _key(value: object, name: str) -> tuple[type, object]:
    key = (type(value), value)
    try:
        hash(key)
    except TypeError as error:
        raise TypeError(f"{name} must be an immutable hashable native value") from error
    return key

@dataclass(frozen=True)
class FixedDelay:
    """Retry after the same positive delay every time."""
    delay: timedelta
    kind: Literal["fixed"] = field(default="fixed", init=False)

@dataclass(frozen=True)
class ExponentialBackoff:
    """Double the delay from ``initial`` after each failed attempt, capped at ``maximum``."""
    initial: timedelta
    maximum: timedelta
    kind: Literal["exponential"] = field(default="exponential", init=False)

@dataclass(frozen=True)
class RetryAt:
    """Request a wake after ``delay``; the caller schedules it."""
    delay: timedelta
    kind: Literal["at"] = field(default="at", init=False)

@dataclass(frozen=True)
class RetryExhausted:
    """The attempt bound or the elapsed window is reached; do not retry."""
    kind: Literal["exhausted"] = field(default="exhausted", init=False)

RetrySchedule: TypeAlias = RetryAt | RetryExhausted

@dataclass(frozen=True, kw_only=True)
class RetryPolicy:
    """A bounded fixed or capped exponential policy, validated by the kernel.

    ``max_attempts`` counts every attempt, the first included. ``max_elapsed`` is
    required: a ``timedelta`` bounds the time since the first attempt, and ``None``
    explicitly states an indefinite elapsed window (the attempt bound still holds).
    """
    delay: FixedDelay | ExponentialBackoff
    max_attempts: int
    max_elapsed: timedelta | None
    _kernel: _native._RetryPolicy = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        if isinstance(self.delay, FixedDelay):
            delay: dict[str, object] = {"kind": "fixed", "delay": _duration(self.delay.delay, "delay")}
        elif isinstance(self.delay, ExponentialBackoff):
            delay = {"kind": "exponential", "initial": _duration(self.delay.initial, "initial"),
                "maximum": _duration(self.delay.maximum, "maximum")}
        else:
            raise TypeError("delay must be a FixedDelay or an ExponentialBackoff")
        if type(self.max_attempts) is not int:
            raise TypeError("max_attempts must be an integer")
        source = {"delay": delay, "max_attempts": str(self.max_attempts),
            "max_elapsed": None if self.max_elapsed is None else _duration(self.max_elapsed, "max_elapsed")}
        values: dict[str, object] = {"max_attempts": self.max_attempts, "max_elapsed": self.max_elapsed,
            **{name: getattr(self.delay, name) for name in ("delay", "initial", "maximum") if hasattr(self.delay, name)}}
        try:
            object.__setattr__(self, "_kernel", _native._RetryPolicy(json.dumps(source)))
        except _native.NativeFailure as error:
            _raise(error, values)

    def schedule(self, failed_attempts: int, elapsed: timedelta) -> RetrySchedule:
        """The kernel decision after ``failed_attempts`` failures, ``elapsed`` since the first attempt."""
        if type(failed_attempts) is not int:
            raise TypeError("failed_attempts must be an integer")
        source = {"failed_attempts": str(failed_attempts), "elapsed": _duration(elapsed, "elapsed")}
        try:
            result = json.loads(self._kernel.schedule(json.dumps(source)))
        except _native.NativeFailure as error:
            _raise(error, {"failed_attempts": failed_attempts, "elapsed": elapsed})
        if result["kind"] == "exhausted":
            return RetryExhausted()
        return RetryAt(_from_duration(result["delay"]))

@dataclass(frozen=True, eq=False)
class WaitCorrelation(Generic[InstanceT, OperationT]):
    """The identity a reminder must carry back: instance, operation and generation.

    A correlation is a value; rebuild it after a round trip through a scheduler.
    Constructing one grants nothing: a wait set matches it only when all three
    parts equal a current wait. Equality is type-qualified.
    """
    instance: InstanceT
    operation: OperationT
    generation: int

    def __post_init__(self) -> None:
        if type(self.generation) is not int:
            raise TypeError("generation must be an integer")
        hash(self._key)

    @property
    def _key(self) -> tuple[object, ...]:
        return (*_key(self.instance, "instance"), *_key(self.operation, "operation"), self.generation)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, WaitCorrelation):
            return NotImplemented
        return self._key == other._key

    def __hash__(self) -> int:
        return hash(self._key)

@dataclass(frozen=True)
class WakeIntent(Generic[InstanceT, OperationT, PayloadT]):
    """An accepted wait: its correlation, the caller-supplied deadline (UTC) and payload."""
    correlation: WaitCorrelation[InstanceT, OperationT]
    deadline: datetime
    payload: PayloadT

class RetryWaits(Generic[InstanceT, OperationT, PayloadT]):
    """Bounded active waits of one instance incarnation, held in its machine data.

    Keep one value for the instance's lifetime: generations are never reused, so
    stale, cancelled and replaced waits never match again. A correlation of
    another instance never matches. Registration (``mark_registered``) is
    distinct from consumption, and a due wake is consumed exactly once.
    """
    __slots__ = ("_capacity", "_free", "_instance", "_kernel", "_operations", "_payloads", "_values")

    def __init__(self, *, instance: InstanceT, capacity: int) -> None:
        _key(instance, "instance")
        if type(capacity) is not int:
            raise TypeError("capacity must be an integer")
        try:
            native = _native._RetryWaits(capacity if 0 <= capacity < 2**64 else None)
        except _native.NativeFailure as error:
            _raise(error, {})
        self._instance = instance
        self._capacity = capacity
        self._kernel = native
        # bounded: one interned operation and one payload per active wait (<= capacity).
        self._operations: dict[tuple[type, object], int] = {}
        self._values: dict[int, OperationT] = {}
        self._free: list[int] = []
        self._payloads: dict[int, PayloadT] = {}

    @property
    def instance(self) -> InstanceT:
        return self._instance

    @property
    def capacity(self) -> int:
        return self._capacity

    def schedule(self, operation: OperationT, deadline: datetime, payload: PayloadT) -> WaitCorrelation[InstanceT, OperationT]:
        """Accept a wait, or replace this operation's wait with a new generation.

        Raises ``WaitCapacityReachedError`` before any change when every slot is
        occupied. Within an action, the error refuses the whole transition.
        """
        key = _key(operation, "operation")
        source = {"deadline": _instant(deadline, "deadline")}
        index = self._operations.get(key)
        interned = index is None
        if index is None:
            index = self._free.pop() if self._free else len(self._values)
            self._operations[key] = index
            self._values[index] = operation
        previous = None if interned else json.loads(self._kernel.get(index))
        try:
            generation = int(self._kernel.schedule(json.dumps({"operation": index, **source})))
        except _native.NativeFailure as error:
            if interned:
                self._release(index)
            _raise(error, {"deadline": deadline})
        if previous is not None:
            del self._payloads[int(previous["generation"])]
        self._payloads[generation] = payload
        return WaitCorrelation(self._instance, self._values[index], generation)

    def pending(self) -> tuple[WakeIntent[InstanceT, OperationT, PayloadT], ...]:
        """Accepted waits whose scheduler registration is not yet acknowledged."""
        return tuple(self._intent(item) for item in json.loads(self._kernel.pending()))

    def get(self, operation: OperationT) -> WakeIntent[InstanceT, OperationT, PayloadT] | None:
        """The operation's active wait, registered or pending."""
        result = json.loads(self._kernel.get(self._operations.get(_key(operation, "operation"))))
        return None if result is None else self._intent(result)

    def mark_registered(self, correlation: WaitCorrelation[InstanceT, OperationT]) -> bool:
        """Record registration of exactly this current wait; late acknowledgements return False."""
        return self._observe(self._kernel.mark_registered, correlation, None)

    def is_due(self, correlation: WaitCorrelation[InstanceT, OperationT], now: datetime) -> bool:
        """Whether this exact current wait is due at the caller-supplied ``now``."""
        return self._observe(self._kernel.is_due, correlation, now)

    def consume(self, correlation: WaitCorrelation[InstanceT, OperationT],
            now: datetime) -> WakeIntent[InstanceT, OperationT, PayloadT] | None:
        """Consume a due wake once; unknown, early, stale, foreign and duplicate wakes return None."""
        result = json.loads(self._observe(self._kernel.consume, correlation, now))
        return None if result is None else self._remove(result)

    def cancel(self, operation: OperationT) -> WakeIntent[InstanceT, OperationT, PayloadT] | None:
        """End the operation's wait; removing any scheduler registration stays the caller's work."""
        result = json.loads(self._kernel.cancel(self._operations.get(_key(operation, "operation"))))
        return None if result is None else self._remove(result)

    def _observe(self, method: Any, correlation: object, now: datetime | None) -> Any:
        if not isinstance(correlation, WaitCorrelation):
            raise TypeError("correlation must be a WaitCorrelation")
        source: dict[str, object] = {"correlation": {"owner": _same(correlation.instance, self._instance),
            "operation": self._operations.get(_key(correlation.operation, "operation")),
            "generation": str(correlation.generation)},
            "now": None if now is None else _instant(now, "now")}
        try:
            return method(json.dumps(source))
        except _native.NativeFailure as error:
            _raise(error, {"generation": correlation.generation, "now": now})

    def _intent(self, value: dict[str, Any]) -> WakeIntent[InstanceT, OperationT, PayloadT]:
        generation = int(value["generation"])
        return WakeIntent(WaitCorrelation(self._instance, self._values[value["operation"]], generation),
            _from_instant(value["deadline"]), self._payloads[generation])

    def _remove(self, value: dict[str, Any]) -> WakeIntent[InstanceT, OperationT, PayloadT]:
        intent = self._intent(value)
        del self._payloads[intent.correlation.generation]
        self._release(value["operation"])
        return intent

    def _release(self, index: int) -> None:
        operation = self._values.pop(index)
        del self._operations[_key(operation, "operation")]
        self._free.append(index)

    def _copy(self, payloads: dict[int, PayloadT]) -> Self:
        clone = type(self).__new__(type(self))
        clone._instance = self._instance
        clone._capacity = self._capacity
        clone._kernel = self._kernel.clone()
        clone._operations = dict(self._operations)
        clone._values = dict(self._values)
        clone._free = list(self._free)
        clone._payloads = payloads
        return clone

    def __copy__(self) -> Self:
        return self._copy(dict(self._payloads))

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        """Copy the kernel waits and payloads; instance and operation identities are kept."""
        return self._copy(copy.deepcopy(self._payloads, memo))

    def __reduce__(self) -> NoReturn:
        raise TypeError("RetryWaits holds a native kernel value and cannot be pickled")

    def __repr__(self) -> str:
        return f"RetryWaits(instance={self._instance!r}, capacity={self._capacity}, active={len(self._payloads)})"
