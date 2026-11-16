"""Kernel-decided retry policies and correlated waits with native durations, instants and payloads."""
import copy
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from enum import Enum, IntEnum, auto
import pickle
import unittest
from status_machina import (
    Action, CallbackError, CapBelowInitialError, Condition, Definition, Disposition, Event,
    ExponentialBackoff, FixedDelay, Ignored, InvalidWaitCapacityError, LimitError, RetryAt, RetryError,
    RetryExhausted, RetryPolicy, RetryPolicyError, RetryRangeError, RetryWaitError, RetryWaits, State,
    TooFewAttemptsError, Transition, Transitioned, WaitCapacityReachedError, WaitCorrelation, ZeroDelayError,
)

NOW = datetime(2026, 11, 15, 9, 30, tzinfo=timezone.utc)
HOURS = timedelta(hours=6)

class Op(Enum):
    FETCH = auto()
    PUBLISH = auto()
class Tenant(IntEnum):
    A = 1

def fixed(delay=HOURS, attempts=3, window=None):
    return RetryPolicy(delay=FixedDelay(delay), max_attempts=attempts, max_elapsed=window)

class PolicyTests(unittest.TestCase):
    def test_fixed_multi_hour_delay_is_returned_without_reading_a_clock(self):
        policy = fixed()
        self.assertEqual(policy.schedule(1, timedelta(0)), RetryAt(HOURS))
        self.assertEqual(policy.schedule(2, timedelta(days=30)), RetryAt(HOURS))
        self.assertEqual(policy.schedule(3, timedelta(0)), RetryExhausted())
        self.assertEqual(policy.schedule(0, timedelta(0)), RetryExhausted())
        self.assertEqual(policy.schedule(1, timedelta(0)).kind, "at")

    def test_exponential_backoff_caps_and_its_window_exhausts(self):
        policy = RetryPolicy(delay=ExponentialBackoff(timedelta(minutes=30), timedelta(hours=4)),
            max_attempts=10, max_elapsed=timedelta(hours=12))
        delays = [policy.schedule(attempt, timedelta(0)) for attempt in range(1, 7)]
        self.assertEqual([item.delay for item in delays if isinstance(item, RetryAt)],
            [timedelta(minutes=30), timedelta(hours=1), timedelta(hours=2)] + [timedelta(hours=4)] * 3)
        self.assertEqual(policy.schedule(5, timedelta(hours=8)), RetryAt(timedelta(hours=4)))
        self.assertEqual(policy.schedule(5, timedelta(hours=8, microseconds=1)), RetryExhausted())

    def test_an_explicitly_indefinite_window_still_bounds_attempts(self):
        policy = fixed(window=None)
        self.assertEqual(policy.schedule(2, timedelta.max), RetryAt(HOURS))
        self.assertEqual(policy.schedule(3, timedelta.max), RetryExhausted())
        with self.assertRaises(TypeError):
            RetryPolicy(delay=FixedDelay(HOURS), max_attempts=3)  # type: ignore[call-arg]

    def test_doubling_saturates_at_the_cap_instead_of_overflowing(self):
        policy = RetryPolicy(delay=ExponentialBackoff(timedelta(microseconds=1), timedelta.max),
            max_attempts=2**32 - 1, max_elapsed=None)
        self.assertEqual(policy.schedule(2**32 - 2, timedelta(0)), RetryAt(timedelta.max))
        self.assertEqual(policy.schedule(2**32 - 1, timedelta(0)), RetryExhausted())

    def test_the_kernel_refuses_invalid_policies(self):
        for build, error in ((lambda: fixed(delay=timedelta(0)), ZeroDelayError),
                (lambda: fixed(attempts=1), TooFewAttemptsError),
                (lambda: RetryPolicy(delay=ExponentialBackoff(timedelta(0), HOURS), max_attempts=2,
                    max_elapsed=None), ZeroDelayError),
                (lambda: RetryPolicy(delay=ExponentialBackoff(HOURS, timedelta(hours=1)), max_attempts=2,
                    max_elapsed=None), CapBelowInitialError)):
            with self.assertRaises(error) as caught:
                build()
            self.assertIsInstance(caught.exception, RetryPolicyError)
            self.assertIsInstance(caught.exception, RetryError)

    def test_unrepresentable_values_are_range_errors_with_their_field(self):
        cases = ((lambda: fixed(delay=timedelta(seconds=-1)), "delay"),
            (lambda: fixed(attempts=2**32), "max_attempts"),
            (lambda: fixed(window=-HOURS), "max_elapsed"),
            (lambda: fixed().schedule(-1, timedelta(0)), "failed_attempts"),
            (lambda: fixed().schedule(2**64, timedelta(0)), "failed_attempts"),
            (lambda: fixed().schedule(1, timedelta(microseconds=-1)), "elapsed"))
        for build, name in cases:
            with self.assertRaises(RetryRangeError) as caught:
                build()
            self.assertEqual(caught.exception.context["field"], name)
            self.assertIsInstance(caught.exception, ValueError)
        for build in (lambda: RetryPolicy(delay=HOURS, max_attempts=3, max_elapsed=None),  # type: ignore[arg-type]
                lambda: fixed(attempts=True), lambda: fixed(delay=6),  # type: ignore[arg-type]
                lambda: fixed().schedule(1, 0)):  # type: ignore[arg-type]
            with self.assertRaises(TypeError):
                build()

class WaitTests(unittest.TestCase):
    def setUp(self):
        self.waits: RetryWaits[str, Op, dict] = RetryWaits(instance="order-1", capacity=2)

    def test_registration_is_distinct_from_consumption_and_wakes_consume_once(self):
        payload = {"schedule": "s-1"}
        due = NOW + HOURS
        correlation = self.waits.schedule(Op.FETCH, due, payload)
        self.assertEqual(correlation, WaitCorrelation("order-1", Op.FETCH, 1))
        self.assertEqual([intent.payload for intent in self.waits.pending()], [payload])
        self.assertTrue(self.waits.mark_registered(correlation))
        self.assertEqual(self.waits.pending(), ())
        self.assertFalse(self.waits.is_due(correlation, NOW))
        self.assertIsNone(self.waits.consume(correlation, NOW + timedelta(hours=5, minutes=59)))
        intent = self.waits.consume(correlation, due.astimezone(timezone(timedelta(hours=-5))))
        assert intent is not None
        self.assertIs(intent.payload, payload)
        self.assertEqual(intent.deadline, due)
        self.assertIsNone(self.waits.consume(correlation, due))
        self.assertFalse(self.waits.mark_registered(correlation))
        self.assertIsNone(self.waits.get(Op.FETCH))

    def test_replaced_and_cancelled_wakes_are_stale_forever(self):
        first = self.waits.schedule(Op.FETCH, NOW, "first")
        replacement = self.waits.schedule(Op.FETCH, NOW, "second")
        self.assertLess(first.generation, replacement.generation)
        self.assertFalse(self.waits.mark_registered(first))
        self.assertIsNone(self.waits.consume(first, NOW))
        self.assertEqual(len(self.waits.pending()), 1)
        cancelled = self.waits.cancel(Op.FETCH)
        assert cancelled is not None
        self.assertEqual(cancelled.payload, "second")
        self.assertIsNone(self.waits.cancel(Op.FETCH))
        self.assertIsNone(self.waits.consume(replacement, NOW))
        recreated = self.waits.schedule(Op.FETCH, NOW, "third")
        self.assertLess(replacement.generation, recreated.generation)
        self.assertIsNone(self.waits.consume(replacement, NOW))
        intent = self.waits.consume(recreated, NOW)
        self.assertEqual(intent.payload if intent else None, "third")

    def test_foreign_and_rebuilt_correlations(self):
        other: RetryWaits[object, Op, str] = RetryWaits(instance=Tenant.A, capacity=1)
        mine: RetryWaits[object, Op, str] = RetryWaits(instance=1, capacity=1)
        foreign = other.schedule(Op.FETCH, NOW, "theirs")
        current = mine.schedule(Op.FETCH, NOW, "mine")
        self.assertEqual(foreign.generation, current.generation)
        # Tenant.A == 1, but type-qualified identity keeps the instances apart.
        self.assertIsNone(mine.consume(foreign, NOW))
        self.assertIsNone(mine.consume(WaitCorrelation(1, Op.PUBLISH, current.generation), NOW))
        self.assertFalse(mine.is_due(WaitCorrelation(True, Op.FETCH, current.generation), NOW))
        rebuilt = WaitCorrelation(1, Op.FETCH, current.generation)
        self.assertEqual(hash(rebuilt), hash(current))
        intent = mine.consume(rebuilt, NOW)
        self.assertEqual(intent.payload if intent else None, "mine")
        with self.assertRaises(RetryRangeError) as caught:
            mine.consume(WaitCorrelation(1, Op.FETCH, 2**64), NOW)
        self.assertEqual(caught.exception.context["value"], 2**64)
        with self.assertRaises(TypeError):
            WaitCorrelation(1, Op.FETCH, "1")  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            mine.consume((1, Op.FETCH, 1), NOW)  # type: ignore[arg-type]

    def test_capacity_is_explicit_and_reaching_it_changes_nothing(self):
        for capacity in (0, 4097, 2**64):
            with self.assertRaises(InvalidWaitCapacityError) as caught:
                RetryWaits(instance="i", capacity=capacity)
            self.assertIsInstance(caught.exception, LimitError)
        with self.assertRaises(TypeError):
            RetryWaits(instance="i", capacity=1.0)  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            RetryWaits(instance=[], capacity=1)
        waits: RetryWaits[str, object, str] = RetryWaits(instance="i", capacity=1)
        first = waits.schedule(Op.FETCH, NOW, "kept")
        with self.assertRaises(WaitCapacityReachedError) as caught:
            waits.schedule(Op.PUBLISH, NOW, "refused")
        self.assertIsInstance(caught.exception, RetryWaitError)
        self.assertEqual(caught.exception.context["limit"], 1)
        self.assertIsNone(waits.get(Op.PUBLISH))
        self.assertEqual([intent.correlation for intent in waits.pending()], [first])
        waits.cancel(Op.FETCH)
        token = object()
        self.assertEqual(waits.schedule(token, NOW, "next").generation, first.generation + 1)

    def test_instants_are_aware_and_bounded(self):
        with self.assertRaises(ValueError):
            self.waits.schedule(Op.FETCH, datetime(2026, 1, 1), {})
        with self.assertRaises(TypeError):
            self.waits.schedule(Op.FETCH, 0, {})  # type: ignore[arg-type]
        early = datetime(1969, 12, 31, 23, 59, 59, 500000, tzinfo=timezone.utc)
        correlation = self.waits.schedule(Op.FETCH, early, {})
        intent = self.waits.get(Op.FETCH)
        self.assertEqual(intent.deadline if intent else None, early)
        self.assertTrue(self.waits.is_due(correlation, early))

    def test_copies_are_independent_and_keep_identities(self):
        token = object()
        payload = {"n": [1]}
        correlation = self.waits.schedule(token, NOW, payload)  # type: ignore[arg-type]
        clone = copy.deepcopy(self.waits)
        intent = clone.consume(correlation, NOW)
        assert intent is not None
        self.assertIs(intent.correlation.operation, token)
        self.assertEqual(intent.payload, payload)
        self.assertIsNot(intent.payload, payload)
        self.assertIsNotNone(self.waits.get(token))  # type: ignore[arg-type]
        shallow = copy.copy(self.waits)
        shared = shallow.consume(correlation, NOW)
        self.assertIs(shared.payload if shared else None, payload)
        self.assertEqual(len(self.waits.pending()), 1)
        with self.assertRaises(TypeError):
            pickle.dumps(self.waits)

class Phase(Enum):
    RUNNING = auto()
    WAITING = auto()
    FAILED = auto()
class Input(Enum):
    FAIL = auto()
    WAKE = auto()
    CANCEL = auto()

@dataclass(frozen=True)
class Clock:
    """Caller-supplied time: the machine never reads a clock."""
    now: datetime
    elapsed: timedelta = timedelta(0)

@dataclass
class Fetch:
    waits: RetryWaits[str, object, str]
    failures: int = 0
    attempts: int = 1
    wakes: list = field(default_factory=list)

POLICY = RetryPolicy(delay=ExponentialBackoff(timedelta(hours=1), HOURS), max_attempts=4, max_elapsed=timedelta(days=1))

def retry_scheduled(data, payload, facts):
    return isinstance(POLICY.schedule(data.failures + 1, facts.elapsed), RetryAt)

def schedule_wake(data, payload, facts):
    data.failures += 1
    decision = POLICY.schedule(data.failures, facts.elapsed)
    assert isinstance(decision, RetryAt)
    data.waits.schedule(payload, facts.now + decision.delay, f"wake {data.failures}")

def wake_due(data, payload, facts):
    return data.waits.is_due(payload, facts.now)

def consume_wake(data, payload, facts):
    intent = data.waits.consume(payload, facts.now)
    data.wakes.append(intent.payload)
    data.attempts += 1

def cancel_wait(data, payload, facts):
    data.waits.cancel(payload)

def not_due(data, payload, facts):
    return not data.waits.is_due(payload, facts.now)

FETCHER = Definition(id="fetcher", initial=Phase.RUNNING,
    states=(State(Phase.RUNNING), State(Phase.WAITING), State(Phase.FAILED, terminal=True)),
    transitions=(
        Transition(id="retry", source=Phase.RUNNING, event=Input.FAIL, target=Phase.WAITING,
            guard=Condition("retry-left", predicate=retry_scheduled), actions=(Action("schedule", apply=schedule_wake),)),
        Transition(id="give-up", source=Phase.RUNNING, event=Input.FAIL, target=Phase.FAILED,
            guard=Condition("exhausted", negated=Condition("retry-left", predicate=retry_scheduled))),
        Transition(id="wake", source=Phase.WAITING, event=Input.WAKE, target=Phase.RUNNING,
            guard=Condition("due", predicate=wake_due), actions=(Action("consume", apply=consume_wake),)),
        Transition(id="cancel", source=Phase.WAITING, event=Input.CANCEL, target=Phase.RUNNING,
            actions=(Action("cancel", apply=cancel_wait),)),
        Transition(id="fail-another", source=Phase.WAITING, event=Input.FAIL, target=Phase.WAITING,
            internal=True, actions=(Action("schedule-another", apply=schedule_wake),)),
    ),
    dispositions=(
        Disposition("early-or-stale", Phase.WAITING, Input.WAKE, Condition("not-due", predicate=not_due),
            "early, stale or foreign wake"),
        Disposition("duplicate", Phase.RUNNING, Input.WAKE, Condition("always", predicate=lambda *_: True),
            "duplicate wake after consumption"),
    ))

class MachineIntegrationTests(unittest.TestCase):
    def machine(self, capacity=1):
        return FETCHER.instantiate(id="fetch-1", data=Fetch(RetryWaits(instance="fetch-1", capacity=capacity)))

    def fail(self, machine, operation=Op.FETCH, at=NOW, elapsed=timedelta(0)):
        outcome = machine.dispatch(Event(Input.FAIL, operation), facts=Clock(at, elapsed))
        self.assertIsInstance(outcome, Transitioned)
        intents = machine.data.waits.pending()
        return intents[-1].correlation

    def test_a_multi_hour_wake_is_consumed_once_and_duplicates_never_count(self):
        with self.machine() as machine:
            correlation = self.fail(machine)
            intent = machine.data.waits.get(Op.FETCH)
            self.assertEqual(intent.deadline if intent else None, NOW + timedelta(hours=1))
            early = machine.dispatch(Event(Input.WAKE, correlation), facts=Clock(NOW + timedelta(minutes=59)))
            self.assertIsInstance(early, Ignored)
            due = Clock(NOW + timedelta(hours=1))
            self.assertIsInstance(machine.dispatch(Event(Input.WAKE, correlation), facts=due), Transitioned)
            for _ in range(2):
                duplicate = machine.dispatch(Event(Input.WAKE, correlation), facts=due)
                self.assertIsInstance(duplicate, Ignored)
            self.assertEqual((machine.data.attempts, machine.data.wakes), (2, ["wake 1"]))
            second = self.fail(machine, at=due.now, elapsed=timedelta(hours=1))
            self.assertEqual(machine.data.waits.get(Op.FETCH).deadline, due.now + timedelta(hours=2))  # type: ignore[union-attr]
            # The first wake cannot satisfy the second wait: generations are never reused.
            stale = machine.dispatch(Event(Input.WAKE, correlation), facts=Clock(NOW + timedelta(days=2)))
            self.assertIsInstance(stale, Ignored)
            self.assertEqual(machine.data.attempts, 2)
            self.assertGreater(second.generation, correlation.generation)

    def test_cancelled_and_foreign_wakes_are_ignored(self):
        with self.machine() as machine:
            correlation = self.fail(machine)
            foreign = WaitCorrelation("fetch-2", Op.FETCH, correlation.generation)
            later = Clock(NOW + timedelta(days=1))
            self.assertIsInstance(machine.dispatch(Event(Input.WAKE, foreign), facts=later), Ignored)
            self.assertIsInstance(machine.dispatch(Event(Input.CANCEL, Op.FETCH), facts=later), Transitioned)
            self.assertIsNone(machine.data.waits.get(Op.FETCH))
            self.assertIsInstance(machine.dispatch(Event(Input.WAKE, correlation), facts=later), Ignored)
            self.assertEqual(machine.data.attempts, 1)

    def test_a_refused_wait_refuses_the_whole_transition(self):
        with self.machine(capacity=1) as machine:
            correlation = self.fail(machine)
            revision = machine.revision
            with self.assertRaises(CallbackError) as caught:
                machine.dispatch(Event(Input.FAIL, Op.PUBLISH), facts=Clock(NOW))
            self.assertIsInstance(caught.exception.__cause__, WaitCapacityReachedError)
            self.assertEqual(machine.revision, revision)
            self.assertEqual(machine.data.failures, 1)
            self.assertEqual([intent.correlation for intent in machine.data.waits.pending()], [correlation])

    def test_window_exhaustion_takes_the_give_up_transition(self):
        with self.machine() as machine:
            correlation = self.fail(machine)
            machine.dispatch(Event(Input.WAKE, correlation), facts=Clock(NOW + timedelta(hours=1)))
            # The second delay (2 h) would end 25.5 h after the first attempt: past the 1 day window.
            outcome = machine.dispatch(Event(Input.FAIL, Op.FETCH),
                facts=Clock(NOW + timedelta(hours=23, minutes=30), timedelta(hours=23, minutes=30)))
            assert isinstance(outcome, Transitioned)
            self.assertEqual(outcome.record.transition, "give-up")
            self.assertIs(machine.state, Phase.FAILED)
            self.assertEqual((machine.data.failures, machine.data.waits.pending()), (1, ()))

if __name__ == "__main__":
    unittest.main()
