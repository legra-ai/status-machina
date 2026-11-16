import assert from 'node:assert/strict';
import { once } from 'node:events';
import test from 'node:test';
import { MessageChannel, Worker, threadId } from 'node:worker_threads';
import { Definition } from '@legra-ai/status-machina';

const fixture = new URL('./workers/machine-worker.mjs', import.meta.url);
const Phase = Object.freeze({ Open: Symbol('open'), Closed: Symbol('closed') });
const Input = Object.freeze({ Close: Symbol('close') });

function local(observed = new Set()) {
  return new Definition({ id: Symbol('main definition'), initial: Phase.Open,
    states: [{ id: Phase.Open }, { id: Phase.Closed }],
    transitions: [{ id: Symbol(), source: Phase.Open, event: Input.Close, target: Phase.Closed,
      guard: { id: Symbol(), predicate() { observed.add(threadId); return true; } } }],
    snapshot: value => { observed.add(threadId); return { ...value }; } });
}

// Buffer every message and the exit event from the moment the worker starts,
// so a message or exit delivered before the test awaits it is never lost.
function start(workerData) {
  const worker = new Worker(fixture, { workerData });
  const messages = [];
  const waiting = [];
  let failed;
  const deliver = () => {
    for (let index = 0; index < waiting.length;) {
      const waiter = waiting[index];
      const found = messages.findIndex(message => message.kind === waiter.kind);
      if (failed !== undefined) { waiting.splice(index, 1); waiter.reject(failed); }
      else if (found >= 0) { waiting.splice(index, 1); waiter.resolve(messages.splice(found, 1)[0]); }
      else index++;
    }
  };
  worker.on('message', message => { messages.push(message); deliver(); });
  worker.on('error', error => { failed = error; deliver(); });
  const exited = once(worker, 'exit').then(([code]) => code);
  return { worker, threadId: worker.threadId, exited, next: kind => new Promise((resolve, reject) => { waiting.push({ kind, resolve, reject }); deliver(); }) };
}

async function holdThenTerminate(mode, options) {
  const { worker, next } = start({ mode, ...options });
  const holding = await next('holding');
  assert.equal(holding.count, options.machines * 2);
  if (mode === 'hold-spinning') await next('spinning');
  assert.equal(await worker.terminate(), 1);
}

test('each worker creates and runs its own machines with callbacks bound to its thread', { timeout: 60_000 }, async () => {
  const rounds = 64;
  const started = Array.from({ length: 4 }, (_, index) => start({ mode: 'run', rounds, label: `worker-${index}` }));
  const observed = new Set();
  const machine = local(observed).instantiate({ id: Symbol(), data: { owner: 'main' } });
  assert.equal(machine.dispatch({ kind: Input.Close, payload: null }).kind, 'transitioned');
  const results = await Promise.all(started.map(({ next }) => next('result')));
  assert.deepEqual(await Promise.all(started.map(({ exited }) => exited)), [0, 0, 0, 0]);
  assert.deepEqual([...observed], [threadId]);
  assert.deepEqual(machine.data, { owner: 'main' });
  machine.dispose();
  const threads = new Set();
  results.forEach((result, index) => {
    assert.equal(result.threadId, started[index].threadId);
    assert.notEqual(result.threadId, threadId);
    threads.add(result.threadId);
    assert.deepEqual(result.observed, [result.threadId]);
    assert.equal(result.revision, BigInt(rounds * 2));
    assert.equal(result.state, 'open');
    assert.equal(result.causeRetained, true);
    assert.deepEqual(result.entries, Array.from({ length: rounds }, (_, round) => [result.threadId, `worker-${index}:${round}`]));
  });
  assert.equal(threads.size, results.length);
});

test('definitions, machines and decisions cannot be cloned into another thread', { timeout: 60_000 }, async () => {
  const definition = local();
  const machine = definition.instantiate({ id: Symbol(), data: { owner: 'main' } });
  const decision = machine.evaluate({ kind: Input.Close, payload: null });
  const { port1, port2 } = new MessageChannel();
  try {
    for (const handle of [definition, machine, decision, { nested: [machine] }]) {
      assert.throws(() => port1.postMessage(handle), { name: 'DataCloneError' });
      assert.throws(() => new Worker(fixture, { workerData: { mode: 'run', rounds: 1, label: 'never', handle } }),
        { name: 'DataCloneError' });
    }
  } finally { port1.close(); port2.close(); }
  assert.equal(machine.accept(decision).kind, 'transitioned');
  assert.equal(machine.state, Phase.Closed);
  machine.dispose();
  const { exited, next } = start({ mode: 'reject' });
  const rejected = await next('rejected');
  assert.equal(await exited, 0);
  assert.deepEqual(rejected.failures, ['DataCloneError', 'DataCloneError', 'DataCloneError']);
  assert.equal(rejected.state, 'closed');
});

test('terminating workers that hold machines and decisions neither crashes nor hangs', { timeout: 120_000 }, async () => {
  for (const mode of ['hold-idle', 'hold-spinning']) {
    for (let round = 0; round < 4; round++) await holdThenTerminate(mode, { machines: 32, payloadBytes: 1_024 });
  }
  const machine = local().instantiate({ id: Symbol(), data: { owner: 'main' } });
  assert.equal(machine.dispatch({ kind: Input.Close, payload: null }).kind, 'transitioned');
  machine.dispose();
});

test('repeated worker termination with live handles releases their native resources', { timeout: 180_000 }, async () => {
  const options = { machines: 64, payloadBytes: 128 * 1_024 };
  for (let round = 0; round < 4; round++) await holdThenTerminate('hold-idle', options);
  const baseline = process.memoryUsage().rss;
  for (let round = 0; round < 16; round++) await holdThenTerminate(round % 2 === 0 ? 'hold-idle' : 'hold-spinning', options);
  const growth = process.memoryUsage().rss - baseline;
  // Each worker retains about 16 MiB of machine data and pending decisions,
  // so leaking even a third of the 16 terminated workers exceeds this bound.
  assert.ok(growth < 96 * 1_024 * 1_024, `resident memory grew by ${growth} bytes`);
});
