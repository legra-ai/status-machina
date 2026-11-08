import assert from 'node:assert/strict';
import test from 'node:test';
import { Definition, CapabilityStatus, BehaviorRef, BehaviorRegistry, BehaviorRole, CallbackError, DefinitionError } from '../index.mjs';
const open = Symbol('open'); const edit = Symbol('edit');
function definition(evaluate, behaviors) {
  return new Definition({ id: Symbol(), initial: open, states: [{ id: open }], transitions: [],
    capabilities: [{ id: edit, evaluate }], behaviors });
}
test('capabilities preserve native identities and observe detached data and facts', () => {
  const identity = Symbol(); const facts = [1];
  const machine = definition((state, data, observed) => {
    assert.equal(state, open); data.push(99); observed.push(99); return CapabilityStatus.Available;
  }).instantiate({ id: identity, data: [1] });
  const report = machine.evaluateCapability(edit, facts);
  assert.deepEqual(report, { capability: edit, instance: identity, revision: 0n, status: CapabilityStatus.Available });
  assert.ok(Object.isFrozen(report)); assert.deepEqual(machine.data, [1]); assert.deepEqual(facts, [1]);
  assert.equal(machine.revision, 0n); machine.dispose();
});
test('capabilities bind exact Story and implementation identities', () => {
  const reference = new BehaviorRef(Symbol('story'), Symbol('implementation'));
  const behaviors = new BehaviorRegistry({ implementations: [{ reference, role: BehaviorRole.Capability,
    callback: () => CapabilityStatus.Unknown }] });
  const machine = definition(reference, behaviors).instantiate({ id: Symbol(), data: null });
  assert.equal(machine.evaluateCapability(edit).status, CapabilityStatus.Unknown); machine.dispose();
  assert.throws(() => definition(new BehaviorRef(reference.story, Symbol()), behaviors), DefinitionError);
});
test('capability errors retain the original cause and do not become denial', () => {
  const failure = new Error('policy failed');
  const machine = definition(() => { throw failure; }).instantiate({ id: Symbol(), data: null });
  assert.throws(() => machine.evaluateCapability(edit), error => error instanceof CallbackError &&
    error.cause === failure && error.context.phase === 'capability');
  assert.equal(machine.revision, 0n); machine.dispose();
});
test('invalid status, unknown capability and duplicate capability IDs fail', () => {
  const machine = definition(() => false).instantiate({ id: Symbol(), data: null });
  assert.throws(() => machine.evaluateCapability(edit), CallbackError);
  assert.throws(() => machine.evaluateCapability(Symbol()), DefinitionError); machine.dispose();
  assert.throws(() => new Definition({ id: Symbol(), initial: open, states: [{ id: open }], transitions: [],
    capabilities: [{ id: edit, evaluate: () => CapabilityStatus.Available },
      { id: edit, evaluate: () => CapabilityStatus.Unavailable }] }), DefinitionError);
});
