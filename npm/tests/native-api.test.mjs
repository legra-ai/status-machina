import assert from 'node:assert/strict';
import test from 'node:test';
import { ActionError, CallbackError, Definition, DisposedError, InvariantError } from '../index.mjs';

const Phase = Object.freeze({ Open: Symbol('open'), Closed: Symbol('closed') });
const Input = Object.freeze({ Close: Symbol('close'), Unknown: Symbol('close') });
class Counter { constructor(values) { this.values = values; } }
function graph(options = {}) {
  return new Definition({ id: Symbol(), initial: Phase.Open,
    states: [{ id: Phase.Open }, { id: Phase.Closed, terminal: true, invariant: options.invariant }],
    transitions: [{ id: Symbol(), source: Phase.Open, event: Input.Close, target: Phase.Closed,
      guard: options.guard, actions: options.actions, effects: options.effects }],
    snapshot: value => new Counter([...value.values]),
  });
}

test('native symbol identities and application classes survive dispatch and detached inspection', () => {
  const definition = graph({ actions: [{ id: Symbol(), apply(data, event, facts) { data.values.push(event + facts); } }] });
  const original = new Counter([1]);
  const machine = definition.instantiate({ id: Symbol(), data: original });
  const result = machine.dispatch({ kind: Input.Close, payload: 2 }, 3);
  assert.equal(result.kind, 'transitioned');
  assert.equal(machine.state, Phase.Closed);
  assert.ok(machine.data instanceof Counter);
  assert.deepEqual(machine.data.values, [1, 5]);
  assert.deepEqual(original.values, [1]);
  const copy = machine.data;
  copy.values.push(99);
  assert.deepEqual(machine.data.values, [1, 5]);
  assert.equal(machine.revision, 1n);
  machine.dispose();
  assert.throws(() => machine.state, DisposedError);
});

test('a thrown guard retains the exact JavaScript cause and cannot become refusal', () => {
  const failure = new TypeError('domain guard failed');
  let called = false;
  const definition = graph({ guard: { id: Symbol(), predicate(data) { data.values.push(99); throw failure; } },
    actions: [{ id: Symbol(), apply() { called = true; } }] });
  const machine = definition.instantiate({ id: Symbol(), data: new Counter([1]) });
  assert.throws(() => machine.dispatch({ kind: Input.Close, payload: null }), error => {
    assert.ok(error instanceof CallbackError);
    assert.equal(error.cause, failure);
    return true;
  });
  assert.equal(machine.state, Phase.Open);
  assert.deepEqual(machine.data.values, [1]);
  assert.equal(machine.revision, 0n);
  assert.equal(called, false);
  machine.dispose();
});

test('retaining an action argument cannot mutate accepted data', () => {
  let retained;
  const machine = graph({ actions: [{ id: Symbol(), apply(data) { retained = data; data.values.push(2); } }] })
    .instantiate({ id: Symbol(), data: new Counter([1]) });
  machine.dispatch({ kind: Input.Close, payload: null });
  retained.values.push(99);
  assert.deepEqual(machine.data.values, [1, 2]);
  machine.dispose();
});

test('equal symbol labels do not collapse event identity and invariants are atomic', () => {
  const machine = graph({ invariant: data => data.values.length > 0 })
    .instantiate({ id: Symbol(), data: new Counter([]) });
  assert.equal(machine.dispatch({ kind: Input.Unknown, payload: null }).kind, 'refused');
  assert.throws(() => machine.dispatch({ kind: Input.Close, payload: null }), InvariantError);
  assert.equal(machine.state, Phase.Open);
  assert.equal(machine.revision, 0n);
  machine.dispose();
});

test('immutable BigInt data can be replaced without number conversion', () => {
  const id = Symbol('state');
  const event = Symbol('increment');
  const machine = new Definition({ id: Symbol(), initial: id, states: [{ id }],
    transitions: [{ id: Symbol(), source: id, event, target: id, internal: true,
      actions: [{ id: Symbol(), update: data => data + 1n }] }] })
    .instantiate({ id: Symbol(), data: 2n ** 80n });
  machine.dispatch({ kind: event, payload: null });
  assert.equal(machine.data, 2n ** 80n + 1n);
  machine.dispose();
});
