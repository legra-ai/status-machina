"""Caller-owned synchronous scope-exit reporting, the native ``EventOnDrop``.

Rust delivers the event when the guard is dropped; Python's deterministic scope
is the ``with`` block, so the guard delivers on ``__exit__``. Garbage collection
never delivers: finalization is not a correctness mechanism.
"""
from __future__ import annotations

import inspect
from collections.abc import Callable
from types import TracebackType
from typing import Generic

from typing_extensions import Self, TypeVar

from ._context import is_asynchronous
from .errors import DisposedError

EventT = TypeVar("EventT")

class EventOnDrop(Generic[EventT]):
    """Deliver one event to a synchronous handler when an unfinished operation leaves its ``with`` block.

    On explicit completion, dispatch the completion event first, then ``disarm()``.
    An armed guard also delivers on early return and on an exception; the
    exception keeps propagating, and an exception raised by the handler replaces
    it with the original as its ``__context__``, as for any ``__exit__``. The
    handler must return ``None``: it handles every dispatch error and emitted
    effect itself, and must not start asynchronous cleanup. Construction performs
    no dispatch; the guard holds no machine, lock or background work.
    """
    __slots__ = ("_armed", "_entered", "_event", "_handler")

    def __init__(self, event: EventT, handler: Callable[[EventT], None]) -> None:
        if not callable(handler):
            raise TypeError("handler must be callable")
        if is_asynchronous(handler):
            raise TypeError("EventOnDrop handlers must be synchronous")
        self._event = event
        self._handler = handler
        self._armed = True
        self._entered = False

    @property
    def armed(self) -> bool:
        return self._armed

    @property
    def event(self) -> EventT | None:
        """The armed event, or ``None`` after disarming."""
        return self._event if self._armed else None

    @event.setter
    def event(self, value: EventT) -> None:
        """Update the armed event's correlation after accepted progress."""
        if not self._armed:
            raise DisposedError("A disarmed EventOnDrop cannot be rearmed")
        self._event = value

    def disarm(self) -> None:
        """Release the event and handler without delivering; repeated calls do nothing."""
        self._armed = False
        self._event = None  # type: ignore[assignment]
        self._handler = None  # type: ignore[assignment]

    def __enter__(self) -> Self:
        if self._entered:
            raise DisposedError("An EventOnDrop guards exactly one scope")
        self._entered = True
        return self

    def __exit__(self, kind: type[BaseException] | None, error: BaseException | None,
            traceback: TracebackType | None) -> None:
        if not self._armed:
            return
        event, handler = self._event, self._handler
        self.disarm()
        result = handler(event)
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("EventOnDrop handlers must return synchronously, not an awaitable")
        if result is not None:
            raise TypeError("EventOnDrop handlers must return None; handle every outcome explicitly")
