// Run with: node examples/retry.mjs (after installing @legra-ai/status-machina).
// A fetch retries with capped exponential backoff. The caller owns the clock and
// the scheduler: the machine only records which wake it accepts.
import { Definition, Duration, ExponentialBackoff, RetryPolicy, RetryWaits } from '@legra-ai/status-machina';

const Phase = Object.freeze({ Running: Symbol('running'), Waiting: Symbol('waiting') });
const Input = Object.freeze({ Fail: Symbol('fail'), Wake: Symbol('wake') });
const policy = new RetryPolicy({ delay: new ExponentialBackoff({ initial: Duration.ofHours(1), maximum: Duration.ofHours(4) }),
  maxAttempts: 5, maxElapsed: Duration.ofDays(1) });

const fetch = new Definition({ id: 'fetch', initial: Phase.Running, states: [{ id: Phase.Running }, { id: Phase.Waiting }],
  transitions: [
    { id: 'retry', source: Phase.Running, event: Input.Fail, target: Phase.Waiting,
      guard: { id: 'retry-left', predicate: (data, payload, clock) => policy.schedule(data.failures + 1, clock.elapsed).kind === 'at' },
      actions: [{ id: 'schedule', apply(data, payload, clock) {
        data.failures += 1;
        const { delay } = policy.schedule(data.failures, clock.elapsed);
        data.waits.schedule('fetch', delay.after(clock.now), `timer-${data.failures}`);
      } }] },
    { id: 'wake', source: Phase.Waiting, event: Input.Wake, target: Phase.Running,
      guard: { id: 'due', predicate: (data, correlation, clock) => data.waits.isDue(correlation, clock.now) },
      actions: [{ id: 'consume', apply(data, correlation, clock) { data.waits.consume(correlation, clock.now); data.attempts += 1; } }] },
  ],
  dispositions: [{ id: 'duplicate', source: Phase.Running, event: Input.Wake, reason: 'wake already consumed',
    condition: { id: 'always', predicate: () => true } }] });

const start = new Date(Date.UTC(2026, 10, 15, 9));
const hhmm = date => date.toISOString().slice(11, 16);
const machine = fetch.instantiate({ id: 'fetch-1', data: { waits: new RetryWaits({ instance: 'fetch-1', capacity: 1 }), failures: 0, attempts: 1 } });
try {
  let now = start;
  const clock = () => ({ now, elapsed: Duration.ofMilliseconds(now.getTime() - start.getTime()) });
  for (let round = 0; round < 2; round += 1) {
    machine.dispatch({ kind: Input.Fail, payload: undefined }, clock());
    const [intent] = machine.data.waits.pending();
    // The caller registers intent.payload with its own scheduler, which later
    // returns the correlation unchanged; here the wake arrives on time, twice.
    console.log(`wait ${intent.payload} until ${hhmm(intent.deadline)}`);
    now = intent.deadline;
    for (const delivery of ['wake', 'duplicate']) {
      console.log(`${delivery}: ${machine.dispatch({ kind: Input.Wake, payload: intent.correlation }, clock()).kind}`);
    }
  }
  console.log(`attempts: ${machine.data.attempts}`);
} finally {
  machine.dispose();
}
