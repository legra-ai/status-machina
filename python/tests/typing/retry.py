"""Static contracts for retry policies, schedules, correlations and wait sets."""
from datetime import datetime, timedelta, timezone
from enum import Enum, auto
from typing_extensions import assert_type
from status_machina import (
    ExponentialBackoff, FixedDelay, RetryAt, RetryPolicy, RetrySchedule, RetryWaits, WaitCorrelation, WakeIntent,
)

class Op(Enum):
    FETCH = auto()

now = datetime(2026, 1, 1, tzinfo=timezone.utc)
policy = RetryPolicy(delay=ExponentialBackoff(timedelta(minutes=1), timedelta(hours=6)), max_attempts=8,
    max_elapsed=None)
decision = policy.schedule(1, timedelta(0))
assert_type(decision, RetrySchedule)
if decision.kind == "at":
    assert_type(decision, RetryAt)
    assert_type(decision.delay, timedelta)
waits: RetryWaits[str, Op, int] = RetryWaits(instance="order-1", capacity=4)
correlation = waits.schedule(Op.FETCH, now, 7)
assert_type(correlation, WaitCorrelation[str, Op])
assert_type(correlation.generation, int)
intent = waits.consume(correlation, now)
assert_type(intent, WakeIntent[str, Op, int] | None)
if intent is not None:
    assert_type(intent.payload, int)
    assert_type(intent.deadline, datetime)

RetryPolicy(delay=FixedDelay(timedelta(seconds=1)), max_attempts=3)  # type: ignore[call-arg]
RetryPolicy(delay=FixedDelay(1), max_attempts=3, max_elapsed=None)  # type: ignore[arg-type]
RetryPolicy(delay=timedelta(seconds=1), max_attempts=3, max_elapsed=None)  # type: ignore[arg-type]
policy.schedule(1, 30)  # type: ignore[arg-type]
waits.schedule(Op.FETCH, now, "seven")  # type: ignore[arg-type]
waits.schedule("fetch", now, 7)  # type: ignore[arg-type]
waits.consume((("order-1", Op.FETCH, 1)), now)  # type: ignore[arg-type]
waits.is_due(correlation, 0)  # type: ignore[arg-type]
RetryWaits(instance="order-1")  # type: ignore[call-arg]
