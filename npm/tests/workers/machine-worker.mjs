// Worker-thread fixture for workers.test.mjs: every machine below is created,
// evaluated and disposed inside this worker's own Node environment.
import { parentPort, threadId, workerData } from 'node:worker_threads';
import { CallbackError, Definition } from '../../index.mjs';

const Phase = Object.freeze({ Open: Symbol('open'), Closed: Symbol('closed') });
const Input = Object.freeze({ Close: Symbol('close'), Reopen: Symbol('reopen'), Spin: Symbol('spin') });
class Ledger { constructor(entries) { this.entries = entries; } }
class WorkerFailure extends Error {}

function definition(observed) {
  const note = () => { observed.add(threadId); };
  return new Definition({ id: Symbol('worker definition'), initial: Phase.Open,
    states: [{ id: Phase.Open, invariant: data => { note(); return data instanceof Ledger; } }, { id: Phase.Closed }],
    transitions: [
      { id: Symbol('close'), source: Phase.Open, event: Input.Close, target: Phase.Closed,
        guard: { id: Symbol('allowed'), predicate(data, payload) { note(); if (payload === 'fail') throw new WorkerFailure('worker guard failed'); return true; } },
        actions: [{ id: Symbol('record'), apply(data, payload) { note(); data.entries.push([threadId, payload]); } }] },
      { id: Symbol('reopen'), source: Phase.Closed, event: Input.Reopen, target: Phase.Open },
      { id: Symbol('spin'), source: Phase.Open, event: Input.Spin, target: Phase.Closed,
        guard: { id: Symbol('spinning'), predicate() { parentPort.postMessage({ kind: 'spinning' }); for (;;); } } },
    ],
    snapshot: value => { note(); return new Ledger(value.entries.map(entry => [...entry])); },
  });
}

function run({ rounds, label }) {
  const observed = new Set();
  const machine = definition(observed).instantiate({ id: Symbol(label), data: new Ledger([]) });
  for (let round = 0; round < rounds; round++) {
    const decision = machine.evaluate({ kind: Input.Close, payload: `${label}:${round}` });
    machine.accept(decision);
    machine.dispatch({ kind: Input.Reopen, payload: null });
  }
  let causeRetained = false;
  try { machine.dispatch({ kind: Input.Close, payload: 'fail' }); }
  catch (error) { causeRetained = error instanceof CallbackError && error.cause instanceof WorkerFailure; }
  const entries = machine.data.entries;
  const result = { kind: 'result', threadId, observed: [...observed], revision: machine.revision,
    state: machine.state === Phase.Open ? 'open' : 'closed', entries, causeRetained };
  machine.dispose();
  return result;
}

function hold({ machines, payloadBytes }) {
  const observed = new Set();
  const graph = definition(observed);
  const held = [];
  for (let index = 0; index < machines; index++) {
    const machine = graph.instantiate({ id: index, data: new Ledger([new Uint8Array(payloadBytes).fill(index & 0xff)]) });
    held.push(machine, machine.evaluate({ kind: Input.Close, payload: index }));
  }
  return { graph, held };
}

function reject() {
  const machine = definition(new Set()).instantiate({ id: Symbol('local'), data: new Ledger([]) });
  const decision = machine.evaluate({ kind: Input.Close, payload: 'kept' });
  const failures = [];
  for (const value of [machine, decision, { nested: machine }]) {
    try { parentPort.postMessage(value); failures.push('posted'); }
    catch (error) { failures.push(error.name); }
  }
  machine.accept(decision);
  return { kind: 'rejected', failures, state: machine.state === Phase.Closed ? 'closed' : 'open' };
}

const mode = workerData.mode;
if (mode === 'run') parentPort.postMessage(run(workerData));
else if (mode === 'reject') parentPort.postMessage(reject());
else if (mode === 'hold-idle') {
  const retained = hold(workerData);
  parentPort.postMessage({ kind: 'holding', count: retained.held.length });
  setInterval(() => retained.held.length, 1_000);
} else if (mode === 'hold-spinning') {
  const retained = hold(workerData);
  parentPort.postMessage({ kind: 'holding', count: retained.held.length });
  retained.graph.instantiate({ id: 'spinner', data: new Ledger([]) }).dispatch({ kind: Input.Spin, payload: null });
} else throw new Error(`Unknown worker fixture mode ${mode}`);
