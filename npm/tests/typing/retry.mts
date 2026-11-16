import {
  Duration, ExponentialBackoff, FixedDelay, RetryPolicy, RetryWaits, WaitCorrelation,
  type RetryAt, type RetrySchedule, type WakeIntent,
} from '@legra-ai/status-machina';

enum Op { Fetch }
const now = new Date(0);
const policy = new RetryPolicy({ delay: new ExponentialBackoff({ initial: Duration.ofMinutes(1), maximum: Duration.ofHours(6) }),
  maxAttempts: 8, maxElapsed: null });
const decision: RetrySchedule = policy.schedule(1, Duration.ZERO);
if (decision.kind === 'at') { const at: RetryAt = decision; const delay: Duration = at.delay; void delay; }
const waits = new RetryWaits<string, Op, number>({ instance: 'order-1', capacity: 4 });
const correlation: WaitCorrelation<string, Op> = waits.schedule(Op.Fetch, now, 7);
const generation: bigint = correlation.generation;
void generation;
const intent: WakeIntent<string, Op, number> | undefined = waits.consume(correlation, now);
if (intent !== undefined) { const payload: number = intent.payload; const deadline: Date = intent.deadline; void [payload, deadline]; }

// @ts-expect-error The elapsed window must be stated: a Duration, or null for indefinite.
new RetryPolicy({ delay: new FixedDelay(Duration.ofSeconds(1)), maxAttempts: 3 });
// @ts-expect-error Delays are Durations, not milliseconds.
new FixedDelay(1000);
// @ts-expect-error A policy needs a delay rule, not a bare Duration.
new RetryPolicy({ delay: Duration.ofSeconds(1), maxAttempts: 3, maxElapsed: null });
// @ts-expect-error Elapsed time is a Duration.
policy.schedule(1, 30);
// @ts-expect-error Payloads keep their declared type.
waits.schedule(Op.Fetch, now, 'seven');
// @ts-expect-error Deadlines are Dates, not epoch numbers.
waits.schedule(Op.Fetch, Date.now(), 7);
// @ts-expect-error A correlation is a WaitCorrelation, not a tuple.
waits.consume(['order-1', Op.Fetch, 1n], now);
// @ts-expect-error Generations are BigInts.
new WaitCorrelation({ instance: 'order-1', operation: Op.Fetch, generation: 1 });
// @ts-expect-error Capacity is explicit.
new RetryWaits({ instance: 'order-1' });
// @ts-expect-error A Duration is not a number.
const millis: number = Duration.ofSeconds(1);
void millis;
