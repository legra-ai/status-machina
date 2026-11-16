import assert from 'node:assert/strict';
import test from 'node:test';
import { MessageChannel } from 'node:worker_threads';
import {
  CallbackError, CapBelowInitialError, Definition, Duration, ExponentialBackoff, FixedDelay, InvalidWaitCapacityError,
  RetryAt, RetryError, RetryExhausted, RetryPolicy, RetryPolicyError, RetryRangeError, RetryWaitError, RetryWaits,
  TooFewAttemptsError, WaitCapacityReachedError, WaitCorrelation, ZeroDelayError,
} from '@legra-ai/status-machina';

const NOW = new Date(Date.UTC(2026, 10, 15, 9, 30));
const HOURS = Duration.ofHours(6);
const at = (base, delay) => delay.after(base);
const Op = Object.freeze({ Fetch: Symbol('fetch'), Publish: Symbol('publish') });
function rejects(fn, Type, check = () => {}) {
  assert.throws(fn, error => { assert.ok(error instanceof Type, `${error.name}: ${error.message}`); check(error); return true; });
}
const fixed = ({ delay = HOURS, maxAttempts = 3, maxElapsed = null } = {}) => new RetryPolicy({ delay: new FixedDelay(delay), maxAttempts, maxElapsed });
const delayOf = schedule => { assert.ok(schedule instanceof RetryAt); return schedule.delay; };

test('durations are exact BigInt nanoseconds over the Rust Duration range', () => {
  assert.equal(Duration.from({ hours: 6, minutes: 30 }).nanoseconds, 23_400_000_000_000n);
  assert.equal(Duration.ofMilliseconds(1500).toString(), 'PT1.5S');
  assert.equal(Duration.MAX.seconds, 0xffffffffffffffffn);
  assert.ok(Duration.ofSeconds(1).equals(Duration.ofMilliseconds(1000)));
  assert.equal(Duration.ofSeconds(1).compare(Duration.ofSeconds(2)), -1);
  assert.equal(Duration.ofNanoseconds(1).after(new Date(0)).getTime(), 1, 'a deadline rounds up, never early');
  rejects(() => Duration.ofSeconds(-1), RetryRangeError, error => assert.equal(error.context.value, -1_000_000_000n));
  rejects(() => Duration.MAX.plus(Duration.ofNanoseconds(1)), RetryRangeError);
  rejects(() => Duration.ofDays(200_000_000).after(NOW), RetryRangeError);
  rejects(() => Duration.ofSeconds(1.5), TypeError);
  rejects(() => Duration.from({ weeks: 1 }), TypeError);
  rejects(() => new Duration(5), TypeError);
});

test('a fixed multi-hour delay is returned without reading a clock', () => {
  const policy = fixed();
  assert.ok(delayOf(policy.schedule(1, Duration.ZERO)).equals(HOURS));
  assert.ok(delayOf(policy.schedule(2, Duration.ofDays(30))).equals(HOURS));
  assert.ok(policy.schedule(3, Duration.ZERO) instanceof RetryExhausted);
  assert.equal(policy.schedule(0, Duration.ZERO).kind, 'exhausted');
});

test('exponential backoff caps and its window exhausts', () => {
  const policy = new RetryPolicy({ delay: new ExponentialBackoff({ initial: Duration.ofMinutes(30), maximum: Duration.ofHours(4) }),
    maxAttempts: 10, maxElapsed: Duration.ofHours(12) });
  assert.deepEqual([1, 2, 3, 4, 5].map(attempt => delayOf(policy.schedule(attempt, Duration.ZERO)).toString()),
    ['PT1800S', 'PT3600S', 'PT7200S', 'PT14400S', 'PT14400S']);
  assert.equal(policy.schedule(5, Duration.ofHours(8)).kind, 'at');
  assert.equal(policy.schedule(5, Duration.ofHours(8).plus(Duration.ofNanoseconds(1))).kind, 'exhausted');
});

test('an explicitly indefinite window still bounds attempts, and overflow exhausts instead of wrapping', () => {
  const indefinite = fixed({ maxElapsed: null });
  assert.equal(indefinite.schedule(2, Duration.MAX).kind, 'at');
  assert.equal(indefinite.schedule(3, Duration.MAX).kind, 'exhausted');
  rejects(() => new RetryPolicy({ delay: new FixedDelay(HOURS), maxAttempts: 3 }), TypeError);
  // elapsed + delay overflows Rust's Duration: the kernel exhausts.
  assert.equal(fixed({ maxElapsed: Duration.MAX }).schedule(1, Duration.MAX).kind, 'exhausted');
  const saturating = new RetryPolicy({ delay: new ExponentialBackoff({ initial: Duration.ofNanoseconds(1), maximum: Duration.MAX }),
    maxAttempts: 0xffffffff, maxElapsed: null });
  assert.ok(delayOf(saturating.schedule(100, Duration.ZERO)).equals(Duration.MAX));
  assert.equal(delayOf(saturating.schedule(64, Duration.ZERO)).nanoseconds, 1n << 63n);
});

test('the kernel refuses invalid policies and unrepresentable counts', () => {
  for (const [build, Type] of [[() => fixed({ delay: Duration.ZERO }), ZeroDelayError], [() => fixed({ maxAttempts: 1 }), TooFewAttemptsError],
    [() => new RetryPolicy({ delay: new ExponentialBackoff({ initial: HOURS, maximum: Duration.ofHours(1) }), maxAttempts: 2, maxElapsed: null }), CapBelowInitialError]]) {
    rejects(build, Type, error => { assert.ok(error instanceof RetryPolicyError); assert.ok(error instanceof RetryError); });
  }
  rejects(() => fixed({ maxAttempts: 2 ** 32 }), RetryRangeError, error => {
    assert.equal(error.context.field, 'maxAttempts'); assert.equal(error.context.value, 2 ** 32);
  });
  for (const value of [-1, 2 ** 32, 1e21]) rejects(() => fixed().schedule(value, Duration.ZERO), RetryRangeError, error => assert.equal(error.context.field, 'failedAttempts'));
  for (const build of [() => fixed({ maxAttempts: 2.5 }), () => fixed({ maxElapsed: 3 }), () => fixed().schedule(1, 0),
    () => new RetryPolicy({ delay: HOURS, maxAttempts: 3, maxElapsed: null }), () => new FixedDelay(1000), () => fixed({ maxAttempts: '3' })]) rejects(build, TypeError);
});

test('registration is distinct from consumption and a wake is consumed once', () => {
  const waits = new RetryWaits({ instance: 'order-1', capacity: 2 });
  const payload = { schedule: 's-1' };
  const due = at(NOW, HOURS);
  const correlation = waits.schedule(Op.Fetch, due, payload);
  assert.ok(correlation.equals(new WaitCorrelation({ instance: 'order-1', operation: Op.Fetch, generation: 1n })));
  assert.deepEqual(waits.pending().map(intent => intent.payload), [payload]);
  assert.equal(waits.markRegistered(correlation), true);
  assert.deepEqual(waits.pending(), []);
  assert.equal(waits.isDue(correlation, NOW), false);
  assert.equal(waits.consume(correlation, new Date(due.getTime() - 1)), undefined);
  const intent = waits.consume(correlation, due);
  assert.equal(intent.payload, payload);
  assert.equal(intent.deadline.getTime(), due.getTime());
  assert.equal(waits.consume(correlation, due), undefined);
  assert.equal(waits.markRegistered(correlation), false);
  assert.equal(waits.get(Op.Fetch), undefined);
});

test('replaced and cancelled wakes are stale forever', () => {
  const waits = new RetryWaits({ instance: 'order-1', capacity: 1 });
  const first = waits.schedule(Op.Fetch, NOW, 'first');
  const replacement = waits.schedule(Op.Fetch, NOW, 'second');
  assert.ok(first.generation < replacement.generation);
  assert.equal(waits.markRegistered(first), false);
  assert.equal(waits.consume(first, NOW), undefined);
  assert.equal(waits.pending().length, 1);
  assert.equal(waits.cancel(Op.Fetch).payload, 'second');
  assert.equal(waits.cancel(Op.Fetch), undefined);
  const recreated = waits.schedule(Op.Fetch, NOW, 'third');
  assert.ok(replacement.generation < recreated.generation);
  assert.equal(waits.consume(replacement, NOW), undefined);
  assert.equal(waits.consume(recreated, NOW).payload, 'third');
});

test('foreign correlations never match, rebuilt ones match exactly', () => {
  const token = {};
  const other = new RetryWaits({ instance: {}, capacity: 1 });
  const mine = new RetryWaits({ instance: token, capacity: 1 });
  const foreign = other.schedule(Op.Fetch, NOW, 'theirs');
  const current = mine.schedule(Op.Fetch, NOW, 'mine');
  assert.equal(foreign.generation, current.generation);
  assert.equal(mine.consume(foreign, NOW), undefined, 'an equal-looking object token is another instance');
  assert.equal(mine.consume(new WaitCorrelation({ instance: token, operation: Symbol('fetch'), generation: 1n }), NOW), undefined);
  assert.equal(mine.consume(new WaitCorrelation({ instance: token, operation: Op.Fetch, generation: 2n }), NOW), undefined);
  assert.equal(mine.consume(new WaitCorrelation({ instance: token, operation: Op.Fetch, generation: 1n }), NOW).payload, 'mine');
  rejects(() => mine.consume(new WaitCorrelation({ instance: token, operation: Op.Fetch, generation: 1n << 64n }), NOW), RetryRangeError,
    error => assert.equal(error.context.value, 1n << 64n));
  rejects(() => new WaitCorrelation({ instance: token, operation: Op.Fetch, generation: 1 }), TypeError);
  rejects(() => mine.consume({ instance: token, operation: Op.Fetch, generation: 1n }, NOW), TypeError);
  rejects(() => mine.isDue(current, new Date(NaN)), TypeError);
  rejects(() => mine.schedule(Op.Fetch, Date.now(), 1), TypeError);
});

test('capacity is explicit and reaching it changes nothing', () => {
  for (const capacity of [0, 4097, 2 ** 40]) rejects(() => new RetryWaits({ instance: 'i', capacity }), InvalidWaitCapacityError);
  rejects(() => new RetryWaits({ instance: 'i' }), TypeError);
  rejects(() => new RetryWaits({ instance: 'i', capacity: 1.5 }), TypeError);
  const waits = new RetryWaits({ instance: 'i', capacity: 1 });
  const first = waits.schedule(Op.Fetch, NOW, 'kept');
  rejects(() => waits.schedule(Op.Publish, NOW, 'refused'), WaitCapacityReachedError, error => {
    assert.ok(error instanceof RetryWaitError); assert.equal(error.context.limit, 1);
  });
  assert.equal(waits.get(Op.Publish), undefined);
  assert.ok(waits.pending()[0].correlation.equals(first));
  waits.cancel(Op.Fetch);
  assert.equal(waits.schedule(Op.Publish, new Date(-1500), 'next').generation, first.generation + 1n);
  assert.equal(waits.get(Op.Publish).deadline.getTime(), -1500);
});

test('copies are independent, keep identities and refuse structured cloning', () => {
  const waits = new RetryWaits({ instance: 'i', capacity: 1 });
  const token = {};
  const payload = { n: [1] };
  const correlation = waits.schedule(token, NOW, payload);
  const copy = waits.clone(value => structuredClone(value));
  const intent = copy.consume(correlation, NOW);
  assert.equal(intent.correlation.operation, token);
  assert.deepEqual(intent.payload, payload);
  assert.notEqual(intent.payload, payload);
  assert.notEqual(waits.get(token), undefined);
  rejects(() => waits.clone(), TypeError);
  const { port1 } = new MessageChannel();
  assert.throws(() => port1.postMessage(waits), { name: 'DataCloneError' });
  port1.close();
});

const Phase = Object.freeze({ Running: Symbol('running'), Waiting: Symbol('waiting'), Failed: Symbol('failed') });
const Input = Object.freeze({ Fail: Symbol('fail'), Wake: Symbol('wake'), Cancel: Symbol('cancel') });
const POLICY = new RetryPolicy({ delay: new ExponentialBackoff({ initial: Duration.ofHours(1), maximum: HOURS }), maxAttempts: 4, maxElapsed: Duration.ofDays(1) });
const retryLeft = (data, payload, clock) => POLICY.schedule(data.failures + 1, clock.elapsed).kind === 'at';
const FETCHER = new Definition({ id: 'fetcher', initial: Phase.Running,
  states: [{ id: Phase.Running }, { id: Phase.Waiting }, { id: Phase.Failed, terminal: true }],
  transitions: [
    { id: 'retry', source: Phase.Running, event: Input.Fail, target: Phase.Waiting, guard: { id: 'retry-left', predicate: retryLeft },
      actions: [{ id: 'schedule', apply: scheduleWake }] },
    { id: 'give-up', source: Phase.Running, event: Input.Fail, target: Phase.Failed, guard: { id: 'exhausted', not: { id: 'retry-left', predicate: retryLeft } } },
    { id: 'wake', source: Phase.Waiting, event: Input.Wake, target: Phase.Running,
      guard: { id: 'due', predicate: (data, payload, clock) => data.waits.isDue(payload, clock.now) },
      actions: [{ id: 'consume', apply(data, payload, clock) { data.wakes.push(data.waits.consume(payload, clock.now).payload); data.attempts += 1; } }] },
    { id: 'cancel', source: Phase.Waiting, event: Input.Cancel, target: Phase.Running, actions: [{ id: 'cancel', apply(data, payload) { data.waits.cancel(payload); } }] },
    { id: 'fail-another', source: Phase.Waiting, event: Input.Fail, target: Phase.Waiting, internal: true, actions: [{ id: 'schedule-another', apply: scheduleWake }] },
  ],
  dispositions: [
    { id: 'early-or-stale', source: Phase.Waiting, event: Input.Wake, reason: 'early, stale or foreign wake',
      condition: { id: 'not-due', predicate: (data, payload, clock) => !data.waits.isDue(payload, clock.now) } },
    { id: 'duplicate', source: Phase.Running, event: Input.Wake, reason: 'duplicate wake after consumption', condition: { id: 'always', predicate: () => true } },
  ] });
function scheduleWake(data, operation, clock) {
  data.failures += 1;
  const decision = POLICY.schedule(data.failures, clock.elapsed);
  data.waits.schedule(operation, decision.delay.after(clock.now), `wake ${data.failures}`);
}
const clock = (now, elapsed = Duration.ZERO) => ({ now, elapsed });
function machine(capacity = 1) {
  return FETCHER.instantiate({ id: 'fetch-1', data: { waits: new RetryWaits({ instance: 'fetch-1', capacity }), failures: 0, attempts: 1, wakes: [] } });
}
function fail(fetcher, operation = Op.Fetch, now = NOW, elapsed = Duration.ZERO) {
  assert.equal(fetcher.dispatch({ kind: Input.Fail, payload: operation }, clock(now, elapsed)).kind, 'transitioned');
  return fetcher.data.waits.pending().at(-1).correlation;
}

test('a machine consumes a multi-hour wake once; duplicates and stale wakes never count', () => {
  const fetcher = machine();
  try {
    const correlation = fail(fetcher);
    assert.equal(fetcher.data.waits.get(Op.Fetch).deadline.getTime(), at(NOW, Duration.ofHours(1)).getTime());
    assert.equal(fetcher.dispatch({ kind: Input.Wake, payload: correlation }, clock(at(NOW, Duration.ofMinutes(59)))).kind, 'ignored');
    const due = clock(at(NOW, Duration.ofHours(1)));
    assert.equal(fetcher.dispatch({ kind: Input.Wake, payload: correlation }, due).kind, 'transitioned');
    for (let index = 0; index < 2; index += 1) assert.equal(fetcher.dispatch({ kind: Input.Wake, payload: correlation }, due).kind, 'ignored');
    assert.deepEqual([fetcher.data.attempts, fetcher.data.wakes], [2, ['wake 1']]);
    const second = fail(fetcher, Op.Fetch, due.now, Duration.ofHours(1));
    assert.equal(fetcher.data.waits.get(Op.Fetch).deadline.getTime(), at(due.now, Duration.ofHours(2)).getTime());
    assert.equal(fetcher.dispatch({ kind: Input.Wake, payload: correlation }, clock(at(NOW, Duration.ofDays(2)))).kind, 'ignored');
    assert.equal(fetcher.data.attempts, 2);
    assert.ok(second.generation > correlation.generation);
  } finally { fetcher.dispose(); }
});

test('a machine ignores cancelled and foreign wakes', () => {
  const fetcher = machine();
  try {
    const correlation = fail(fetcher);
    const later = clock(at(NOW, Duration.ofDays(1)));
    const foreign = new WaitCorrelation({ instance: 'fetch-2', operation: Op.Fetch, generation: correlation.generation });
    assert.equal(fetcher.dispatch({ kind: Input.Wake, payload: foreign }, later).kind, 'ignored');
    assert.equal(fetcher.dispatch({ kind: Input.Cancel, payload: Op.Fetch }, later).kind, 'transitioned');
    assert.equal(fetcher.data.waits.get(Op.Fetch), undefined);
    assert.equal(fetcher.dispatch({ kind: Input.Wake, payload: correlation }, later).kind, 'ignored');
    assert.equal(fetcher.data.attempts, 1);
  } finally { fetcher.dispose(); }
});

test('a refused wait refuses the whole transition', () => {
  const fetcher = machine(1);
  try {
    const correlation = fail(fetcher);
    const revision = fetcher.revision;
    rejects(() => fetcher.dispatch({ kind: Input.Fail, payload: Op.Publish }, clock(NOW)), CallbackError,
      error => assert.ok(error.cause instanceof WaitCapacityReachedError));
    assert.equal(fetcher.revision, revision);
    assert.equal(fetcher.data.failures, 1);
    assert.ok(fetcher.data.waits.pending()[0].correlation.equals(correlation));
  } finally { fetcher.dispose(); }
});

test('window exhaustion takes the give-up transition', () => {
  const fetcher = machine();
  try {
    const correlation = fail(fetcher);
    fetcher.dispatch({ kind: Input.Wake, payload: correlation }, clock(at(NOW, Duration.ofHours(1))));
    const late = Duration.from({ hours: 23, minutes: 30 });
    const outcome = fetcher.dispatch({ kind: Input.Fail, payload: Op.Fetch }, clock(at(NOW, late), late));
    assert.equal(outcome.record.transition, 'give-up');
    assert.equal(fetcher.state, Phase.Failed);
    assert.deepEqual([fetcher.data.failures, fetcher.data.waits.pending().length], [1, 0]);
  } finally { fetcher.dispose(); }
});
