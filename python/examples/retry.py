"""Run with: python examples/retry.py (after installing status-machina).

A fetch retries with capped exponential backoff. The caller owns the clock and
the scheduler: the machine only records which wake it accepts.
"""
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from enum import Enum, auto

from status_machina import (
    Action,
    Condition,
    Definition,
    Disposition,
    Event,
    ExponentialBackoff,
    RetryAt,
    RetryPolicy,
    RetryWaits,
    State,
    Transition,
    WaitCorrelation,
)


class Phase(Enum):
    RUNNING = auto()
    WAITING = auto()
class Input(Enum):
    FAIL = auto()
    WAKE = auto()

@dataclass(frozen=True)
class Clock:
    now: datetime
    elapsed: timedelta

@dataclass
class Fetch:
    waits: RetryWaits[str, str, str]
    failures: int = 0
    attempts: int = 1

POLICY = RetryPolicy(delay=ExponentialBackoff(timedelta(hours=1), timedelta(hours=4)), max_attempts=5,
    max_elapsed=timedelta(days=1))

def schedule(data: Fetch, payload: None, facts: Clock) -> None:
    data.failures += 1
    decision = POLICY.schedule(data.failures, facts.elapsed)
    assert isinstance(decision, RetryAt)
    data.waits.schedule("fetch", facts.now + decision.delay, f"timer-{data.failures}")

def consume(data: Fetch, payload: WaitCorrelation[str, str], facts: Clock) -> None:
    data.waits.consume(payload, facts.now)
    data.attempts += 1

FETCH = Definition(id="fetch", initial=Phase.RUNNING, states=(State(Phase.RUNNING), State(Phase.WAITING)),
    transitions=(
        Transition(id="retry", source=Phase.RUNNING, event=Input.FAIL, target=Phase.WAITING,
            guard=Condition("retry-left", predicate=lambda data, payload, facts:
                isinstance(POLICY.schedule(data.failures + 1, facts.elapsed), RetryAt)),
            actions=(Action("schedule", apply=schedule),)),
        Transition(id="wake", source=Phase.WAITING, event=Input.WAKE, target=Phase.RUNNING,
            guard=Condition("due", predicate=lambda data, payload, facts: data.waits.is_due(payload, facts.now)),
            actions=(Action("consume", apply=consume),))),
    dispositions=(Disposition("duplicate", Phase.RUNNING, Input.WAKE,
        Condition("always", predicate=lambda data, payload, facts: True), "wake already consumed"),))

def main() -> None:
    start = datetime(2026, 11, 15, 9, 0, tzinfo=timezone.utc)
    with FETCH.instantiate(id="fetch-1", data=Fetch(RetryWaits(instance="fetch-1", capacity=1))) as machine:
        now = start
        for _ in range(2):
            machine.dispatch(Event(Input.FAIL, None), facts=Clock(now, now - start))
            (intent,) = machine.data.waits.pending()
            # The caller registers intent.payload with its own scheduler, which later
            # returns the correlation unchanged; here the wake arrives on time, twice.
            print(f"wait {intent.payload} until {intent.deadline:%H:%M}")
            now = intent.deadline
            for delivery in ("wake", "duplicate"):
                outcome = machine.dispatch(Event(Input.WAKE, intent.correlation), facts=Clock(now, now - start))
                print(f"{delivery}: {outcome.kind}")
        print(f"attempts: {machine.data.attempts}")

if __name__ == "__main__":
    main()
