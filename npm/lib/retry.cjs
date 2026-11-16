'use strict';
// Retry policies and correlated waits, decided by the kernel's RetryPolicy and
// RetryWaits. Durations are Duration values (exact BigInt nanoseconds over the
// range of Rust's Duration) and instants are Date values. The caller supplies
// every count, elapsed duration, deadline and current time: nothing here reads
// a clock, starts a timer or schedules a wake.
const { markAsUncloneable } = require('node:worker_threads');
const native = require('./native.cjs');
const errors = require('./errors.cjs');
const NANOS = 1_000_000_000n;
const MAX_NANOS = 0xffffffffffffffffn * NANOS + (NANOS - 1n);
const COPY = Symbol('retry waits snapshot');
const failures = Object.freeze({
  'policy/zero_delay': errors.ZeroDelayError, 'policy/too_few_attempts': errors.TooFewAttemptsError,
  'policy/cap_below_initial': errors.CapBelowInitialError, 'waits/invalid_capacity': errors.InvalidWaitCapacityError,
  'waits/capacity_reached': errors.WaitCapacityReachedError, 'waits/generation_exhausted': errors.WaitGenerationExhaustedError,
  'range/out_of_range': errors.RetryRangeError });
const fieldNames = Object.freeze({ max_attempts: 'maxAttempts', max_elapsed: 'maxElapsed', failed_attempts: 'failedAttempts' });
function raise(error, values) {
  if (!errors.isNative(error)) throw error;
  const info = JSON.parse(error.message);
  const Type = info.code === 'retry' ? failures[`${info.family}/${info.reason}`] : undefined;
  if (Type === undefined) throw new Error('Unrecognized native retry failure', { cause: error });
  const context = { reason: info.reason };
  if (info.field !== null) {
    context.field = fieldNames[info.field] ?? info.field;
    if (Object.hasOwn(values, context.field)) context.value = values[context.field];
  }
  if (info.limit !== null) context.limit = info.limit;
  throw new Type(info.message, context);
}
function same(left, right) { return left === right || (left !== left && right !== right); }
function whole(value, label) {
  if (typeof value === 'bigint') return value;
  if (Number.isSafeInteger(value)) return BigInt(value);
  throw new TypeError(`${label} must be a safe integer or a BigInt`);
}
function options(value, allowed, required, label) {
  if (value === null || typeof value !== 'object') throw new TypeError(`${label} requires an options object`);
  for (const key of Reflect.ownKeys(value)) if (!allowed.includes(key)) throw new TypeError(`Unknown ${label} option ${String(key)}`);
  for (const key of required) if (!Object.hasOwn(value, key)) throw new TypeError(`${label} requires ${key}`);
}
const UNITS = Object.freeze({ days: 86_400n * NANOS, hours: 3_600n * NANOS, minutes: 60n * NANOS, seconds: NANOS,
  milliseconds: 1_000_000n, microseconds: 1_000n, nanoseconds: 1n });
/** A non-negative span of time with exact nanoseconds, the range of Rust's Duration. */
class Duration {
  #nanoseconds;
  constructor(nanoseconds) {
    if (typeof nanoseconds !== 'bigint') throw new TypeError('A Duration is built from BigInt nanoseconds');
    if (nanoseconds < 0n || nanoseconds > MAX_NANOS) {
      throw new errors.RetryRangeError('a duration must be non-negative and at most Duration.MAX', { field: 'duration', value: nanoseconds });
    }
    this.#nanoseconds = nanoseconds;
    Object.freeze(this);
  }
  /** Sum whole-unit components, e.g. `Duration.from({ hours: 6, minutes: 30 })`. */
  static from(parts) {
    options(parts, Object.keys(UNITS), [], 'Duration.from');
    let total = 0n;
    for (const [unit, scale] of Object.entries(UNITS)) if (parts[unit] !== undefined) total += whole(parts[unit], unit) * scale;
    return new Duration(total);
  }
  static ofDays(value) { return Duration.from({ days: value }); }
  static ofHours(value) { return Duration.from({ hours: value }); }
  static ofMinutes(value) { return Duration.from({ minutes: value }); }
  static ofSeconds(value) { return Duration.from({ seconds: value }); }
  static ofMilliseconds(value) { return Duration.from({ milliseconds: value }); }
  static ofNanoseconds(value) { return Duration.from({ nanoseconds: value }); }
  get nanoseconds() { return this.#nanoseconds; }
  /** Whole seconds. */
  get seconds() { return this.#nanoseconds / NANOS; }
  /** Nanoseconds below one second. */
  get subsecondNanoseconds() { return Number(this.#nanoseconds % NANOS); }
  equals(other) { return other instanceof Duration && other.nanoseconds === this.#nanoseconds; }
  compare(other) {
    if (!(other instanceof Duration)) throw new TypeError('compare needs a Duration');
    return this.#nanoseconds < other.nanoseconds ? -1 : this.#nanoseconds > other.nanoseconds ? 1 : 0;
  }
  plus(other) {
    if (!(other instanceof Duration)) throw new TypeError('plus needs a Duration');
    return new Duration(this.#nanoseconds + other.nanoseconds);
  }
  /** `instant + this`, rounded up to Date's millisecond so a deadline is never early. */
  after(instant) {
    const milliseconds = BigInt(time(instant, 'instant')) + (this.#nanoseconds + 999_999n) / 1_000_000n;
    const result = new Date(milliseconds > 8_640_000_000_000_000n ? NaN : Number(milliseconds));
    if (Number.isNaN(result.getTime())) throw new errors.RetryRangeError('the deadline is outside the Date range', { field: 'instant', value: instant });
    return result;
  }
  toString() {
    const fraction = this.subsecondNanoseconds === 0 ? '' : `.${String(this.subsecondNanoseconds).padStart(9, '0').replace(/0+$/, '')}`;
    return `PT${this.seconds}${fraction}S`;
  }
  toJSON() { return this.toString(); }
}
Object.defineProperties(Duration, { ZERO: { value: new Duration(0n) }, MAX: { value: new Duration(MAX_NANOS) } });
function duration(value, label) {
  if (!(value instanceof Duration)) throw new TypeError(`${label} must be a Duration`);
  return { seconds: value.seconds.toString(), nanoseconds: value.subsecondNanoseconds };
}
function fromDuration(value) { return new Duration(BigInt(value.seconds) * NANOS + BigInt(value.nanoseconds)); }
function time(value, label) {
  if (!(value instanceof Date)) throw new TypeError(`${label} must be a Date`);
  const milliseconds = Date.prototype.getTime.call(value);
  if (Number.isNaN(milliseconds)) throw new TypeError(`${label} is an invalid Date`);
  return milliseconds;
}
function instant(value, label) {
  const milliseconds = time(value, label);
  const seconds = Math.floor(milliseconds / 1000);
  return { seconds: String(seconds), nanoseconds: (milliseconds - seconds * 1000) * 1_000_000 };
}
function fromInstant(value) { return new Date(Number(BigInt(value.seconds) * 1000n + BigInt(value.nanoseconds / 1_000_000))); }
function attempts(value, label) {
  if (!Number.isInteger(value)) throw new TypeError(`${label} must be an integer`);
  return Number.isSafeInteger(value) ? String(value) : BigInt(value).toString();
}
/** Retry after the same positive delay every time. */
class FixedDelay {
  constructor(delay) {
    duration(delay, 'delay');
    this.kind = 'fixed'; this.delay = delay;
    Object.freeze(this);
  }
}
/** Double the delay from `initial` after each failed attempt, capped at `maximum`. */
class ExponentialBackoff {
  constructor(value) {
    options(value, ['initial', 'maximum'], ['initial', 'maximum'], 'ExponentialBackoff');
    duration(value.initial, 'initial'); duration(value.maximum, 'maximum');
    this.kind = 'exponential'; this.initial = value.initial; this.maximum = value.maximum;
    Object.freeze(this);
  }
}
/** Request a wake after `delay`; the caller schedules it. */
class RetryAt {
  constructor(delay) { this.kind = 'at'; this.delay = delay; Object.freeze(this); }
}
/** The attempt bound or the elapsed window is reached; do not retry. */
class RetryExhausted {
  constructor() { this.kind = 'exhausted'; Object.freeze(this); }
}
/**
 * A bounded fixed or capped exponential policy, validated by the kernel.
 * `maxAttempts` counts every attempt. `maxElapsed` is required: a Duration
 * bounds the time since the first attempt, and `null` explicitly states an
 * indefinite elapsed window (the attempt bound still holds).
 */
class RetryPolicy {
  #native;
  constructor(value) {
    options(value, ['delay', 'maxAttempts', 'maxElapsed'], ['delay', 'maxAttempts', 'maxElapsed'], 'RetryPolicy');
    const { delay, maxAttempts, maxElapsed } = value;
    let rule;
    if (delay instanceof FixedDelay) rule = { kind: 'fixed', delay: duration(delay.delay, 'delay') };
    else if (delay instanceof ExponentialBackoff) rule = { kind: 'exponential', initial: duration(delay.initial, 'initial'), maximum: duration(delay.maximum, 'maximum') };
    else throw new TypeError('delay must be a FixedDelay or an ExponentialBackoff');
    if (maxElapsed !== null && !(maxElapsed instanceof Duration)) throw new TypeError('maxElapsed must be a Duration or null (indefinite)');
    const source = { delay: rule, max_attempts: attempts(maxAttempts, 'maxAttempts'), max_elapsed: maxElapsed === null ? null : duration(maxElapsed, 'maxElapsed') };
    try { this.#native = new native.NativeRetryPolicy(JSON.stringify(source)); }
    catch (error) { raise(error, { maxAttempts, maxElapsed, ...delay }); }
    this.delay = delay; this.maxAttempts = maxAttempts; this.maxElapsed = maxElapsed;
    markAsUncloneable(this);
    Object.freeze(this);
  }
  /** The kernel decision after `failedAttempts` failures, `elapsed` since the first attempt. */
  schedule(failedAttempts, elapsed) {
    const source = { failed_attempts: attempts(failedAttempts, 'failedAttempts'), elapsed: duration(elapsed, 'elapsed') };
    let result;
    try { result = JSON.parse(this.#native.schedule(JSON.stringify(source))); }
    catch (error) { raise(error, { failedAttempts, elapsed }); }
    return result.kind === 'exhausted' ? new RetryExhausted() : new RetryAt(fromDuration(result.delay));
  }
}
/**
 * The identity a reminder must carry back. A correlation is a value; rebuild it
 * after a round trip through a scheduler. Constructing one grants nothing: a
 * wait set matches it only when all three parts equal a current wait.
 */
class WaitCorrelation {
  constructor(value) {
    options(value, ['instance', 'operation', 'generation'], ['instance', 'operation', 'generation'], 'WaitCorrelation');
    if (typeof value.generation !== 'bigint') throw new TypeError('generation must be a BigInt');
    this.instance = value.instance; this.operation = value.operation; this.generation = value.generation;
    Object.freeze(this);
  }
  /** Component-wise Map semantics; object tokens compare by identity. */
  equals(other) {
    return other instanceof WaitCorrelation && same(this.instance, other.instance) && same(this.operation, other.operation)
      && this.generation === other.generation;
  }
}
/** An accepted wait: its correlation, the caller-supplied deadline and payload. */
function wakeIntent(correlation, deadline, payload) { return Object.freeze({ correlation, deadline, payload }); }
/**
 * Bounded active waits of one instance incarnation, held in its machine data.
 * Keep one value for the instance's lifetime: generations are never reused, so
 * stale, cancelled and replaced waits never match again, and a correlation of
 * another instance never matches. The default snapshot copies it with its
 * payloads, keeping instance and operation identities.
 */
class RetryWaits {
  #instance; #capacity; #native; #operations; #values; #free; #payloads;
  constructor(value, internal = undefined) {
    if (internal !== undefined) {
      if (internal.token !== COPY) throw new TypeError('RetryWaits takes one options object');
      this.#adopt(internal);
    } else {
      options(value, ['instance', 'capacity'], ['instance', 'capacity'], 'RetryWaits');
      if (!Number.isInteger(value.capacity)) throw new TypeError('capacity must be an integer');
      const capacity = Number.isSafeInteger(value.capacity) && value.capacity >= 0 && value.capacity <= 0xffffffff ? value.capacity : null;
      let kernel;
      try { kernel = new native.NativeRetryWaits(capacity); }
      catch (error) { raise(error, {}); }
      // bounded: one interned operation and one payload per active wait (<= capacity).
      this.#adopt({ instance: value.instance, capacity: value.capacity, kernel, operations: new Map(), values: new Map(), free: [], payloads: new Map() });
    }
    markAsUncloneable(this);
    Object.freeze(this);
  }
  #adopt(state) {
    this.#instance = state.instance; this.#capacity = state.capacity; this.#native = state.kernel;
    this.#operations = state.operations; this.#values = state.values; this.#free = state.free; this.#payloads = state.payloads;
  }
  get instance() { return this.#instance; }
  get capacity() { return this.#capacity; }
  /**
   * Accept a wait, or replace this operation's wait with a new generation.
   * Throws WaitCapacityReachedError before any change when every slot is
   * occupied; within an action it refuses the whole transition.
   */
  schedule(operation, deadline, payload) {
    const lowered = instant(deadline, 'deadline');
    let index = this.#operations.get(operation);
    const interned = index === undefined;
    if (interned) {
      index = this.#free.length > 0 ? this.#free.pop() : this.#values.size;
      this.#operations.set(operation, index); this.#values.set(index, operation);
    }
    const previous = interned ? null : JSON.parse(this.#native.get(index));
    let generation;
    try { generation = this.#native.schedule(JSON.stringify({ operation: index, deadline: lowered })); }
    catch (error) { if (interned) this.#release(index); raise(error, { deadline }); }
    if (previous !== null) this.#payloads.delete(previous.generation);
    this.#payloads.set(generation, payload);
    return new WaitCorrelation({ instance: this.#instance, operation: this.#values.get(index), generation: BigInt(generation) });
  }
  /** Accepted waits whose scheduler registration is not yet acknowledged. */
  pending() { return Object.freeze(JSON.parse(this.#native.pending()).map(item => this.#intent(item))); }
  /** The operation's active wait, registered or pending. */
  get(operation) {
    const result = JSON.parse(this.#native.get(this.#operations.get(operation) ?? null));
    return result === null ? undefined : this.#intent(result);
  }
  /** Record registration of exactly this current wait; late acknowledgements return false. */
  markRegistered(correlation) { return this.#observe('markRegistered', correlation, undefined); }
  /** Whether this exact current wait is due at the caller-supplied `now`. */
  isDue(correlation, now) { return this.#observe('isDue', correlation, now); }
  /** Consume a due wake once; unknown, early, stale, foreign and duplicate wakes return undefined. */
  consume(correlation, now) {
    const result = JSON.parse(this.#observe('consume', correlation, now));
    return result === null ? undefined : this.#remove(result);
  }
  /** End the operation's wait; removing any scheduler registration stays the caller's work. */
  cancel(operation) {
    const result = JSON.parse(this.#native.cancel(this.#operations.get(operation) ?? null));
    return result === null ? undefined : this.#remove(result);
  }
  /** An independent copy for a custom snapshot function; `copyPayload` copies each payload. */
  clone(copyPayload) {
    if (typeof copyPayload !== 'function') throw new TypeError('clone needs a payload copy function');
    return this[COPY](copyPayload);
  }
  [COPY](copyPayload) {
    const payloads = new Map();
    for (const [generation, payload] of this.#payloads) payloads.set(generation, copyPayload(payload));
    return new RetryWaits(undefined, { token: COPY, instance: this.#instance, capacity: this.#capacity,
      kernel: this.#native.duplicate(), operations: new Map(this.#operations), values: new Map(this.#values),
      free: [...this.#free], payloads });
  }
  #observe(method, correlation, now) {
    if (!(correlation instanceof WaitCorrelation)) throw new TypeError('correlation must be a WaitCorrelation');
    const source = { correlation: { owner: same(correlation.instance, this.#instance),
      operation: this.#operations.get(correlation.operation) ?? null, generation: correlation.generation.toString() },
      now: now === undefined ? null : instant(now, 'now') };
    try { return this.#native[method](JSON.stringify(source)); }
    catch (error) { raise(error, { generation: correlation.generation, now }); }
  }
  #intent(value) {
    const correlation = new WaitCorrelation({ instance: this.#instance, operation: this.#values.get(value.operation), generation: BigInt(value.generation) });
    return wakeIntent(correlation, fromInstant(value.deadline), this.#payloads.get(value.generation));
  }
  #remove(value) {
    const intent = this.#intent(value);
    this.#payloads.delete(value.generation);
    this.#release(value.operation);
    return intent;
  }
  #release(index) {
    this.#operations.delete(this.#values.get(index));
    this.#values.delete(index);
    this.#free.push(index);
  }
}
/** Immutable retry values the default snapshot shares instead of copying. */
function immutable(value) {
  return value instanceof Duration || value instanceof WaitCorrelation || value instanceof RetryPolicy
    || value instanceof FixedDelay || value instanceof ExponentialBackoff || value instanceof RetryAt || value instanceof RetryExhausted;
}
module.exports = { Duration, FixedDelay, ExponentialBackoff, RetryAt, RetryExhausted, RetryPolicy, WaitCorrelation,
  RetryWaits, COPY, immutable };
