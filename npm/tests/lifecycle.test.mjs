import assert from 'node:assert/strict';
import test from 'node:test';
import { createRequire } from 'node:module';
import { BusyError, CallbackError, Definition, DefinitionError, DisposedError,
  SnapshotError, StaleDecisionError } from '../index.mjs';
const require = createRequire(import.meta.url);
const State = Object.freeze({ Ready: Symbol('ready'), Done: Symbol('done') });
const Input = Object.freeze({ Go: Symbol('go') });
function graph(extra = {}) {
  return new Definition({ id: Symbol(), initial: State.Ready,
    states: [{ id: State.Ready }, { id: State.Done }],
    transitions: [{ id: Symbol(), source: State.Ready, event: Input.Go, target: State.Done,
      actions: extra.actions }], snapshot: extra.snapshot, inputSnapshot: extra.inputSnapshot });
}

test('decisions remain unaccepted until checked acceptance and are single-use', () => {
  const machine = graph().instantiate({ id: Symbol(), data: 7 });
  const first = machine.evaluate({ kind: Input.Go, payload: null });
  const stale = machine.evaluate({ kind: Input.Go, payload: null });
  assert.equal(machine.state, State.Ready);
  assert.equal(first.expectedRevision, 0n);
  machine.accept(first);
  assert.throws(() => machine.accept(first), DisposedError);
  assert.throws(() => machine.accept(stale), StaleDecisionError);
  machine.dispose();
});

test('reentrant acceptance refuses before consuming an outstanding decision', () => {
  let decision;
  let machine;
  const definition = graph({ actions: [{ id: Symbol(), apply() { if (decision !== undefined) machine.accept(decision); } }] });
  machine = definition.instantiate({ id: Symbol(), data: 7 });
  decision = machine.evaluate({ kind: Input.Go, payload: null });
  assert.throws(() => machine.dispatch({ kind: Input.Go, payload: null }), error => {
    assert.ok(error instanceof CallbackError);
    assert.ok(error.cause instanceof BusyError);
    return true;
  });
  assert.equal(decision.expectedRevision, 0n);
  assert.equal(machine.revision, 0n);
  decision.dispose(); machine.dispose();
});

test('stale-revision context preserves all uint64 bits as native BigInt', () => {
  const machine = graph().instantiate({ id: Symbol(), data: 7 });
  const expected = 2n ** 63n + 1n;
  assert.throws(() => machine.dispatch({ kind: Input.Go, payload: null }, undefined, { expectedRevision: expected }), error => {
    assert.ok(error instanceof StaleDecisionError);
    assert.equal(error.context.expectedRevision, expected);
    assert.equal(error.context.currentRevision, 0n);
    return true;
  });
  assert.equal(machine.revision, 0n);
  machine.dispose();
});

test('snapshot callbacks cannot be async functions or return Promises', () => {
  assert.throws(() => graph({ snapshot: async value => value }), DefinitionError);
  const definition = graph({ snapshot: value => Promise.resolve(value) });
  assert.throws(() => definition.instantiate({ id: Symbol(), data: 7 }), SnapshotError);
});

test('input snapshot callbacks are independently validated as synchronous', () => {
  assert.throws(() => graph({ inputSnapshot: async value => value }), DefinitionError);
});

test('immutable definition inspection cannot drift after input specifications mutate', () => {
  const state = { id: State.Ready, terminal: false };
  const definition = new Definition({ id: Symbol(), initial: State.Ready, states: [state], transitions: [] });
  state.terminal = true;
  assert.equal(definition.states[0].terminal, false);
  assert.ok(Object.isFrozen(definition.states[0]));
});

test('unknown specification fields fail fast rather than silently changing policy', () => {
  assert.throws(() => new Definition({ id: Symbol(), initial: State.Ready,
    states: [{ id: State.Ready, terminall: true }], transitions: [] }), DefinitionError);
});

test('reclaimed arena slots can be reused without a lifetime dispatch counter limit', () => {
  const { Context } = require('../lib/context.cjs');
  const definition = { _internal: () => ({ compiler: { ids: {} }, snapshot: value => value,
    inputSnapshot: value => value, limits: { maxValues: 2 } }) };
  const context = new Context(definition, Symbol());
  const first = context.allocate({ value: 1 });
  context.native = { liveKeys: () => [] };
  context.sweep();
  assert.equal(context.allocate({ value: 2 }), first);
});

test('CommonJS and ES module entry points expose the same native API classes', () => {
  const common = require('../index.cjs');
  assert.equal(common.Definition, Definition);
  assert.equal(common.CallbackError, CallbackError);
});
