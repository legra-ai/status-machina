import assert from 'node:assert/strict';
import test from 'node:test';
import {
  BehaviorRef, BehaviorRegistry, BehaviorRole, CallbackError, CapabilityStatus, Definition, DefinitionError,
  DefinitionLimitError, DisposedError, DuplicateIdentifierError, DuplicateImplementationError, EventOnDrop,
  InvariantError, PostconditionError, RoleMismatchError, StaleDecisionError, UnknownBehaviorError,
  UnknownCapabilityError, UnknownImplementationError,
} from '@legra-ai/status-machina';

const Phase = Object.freeze({ Draft: Symbol('draft'), Sent: Symbol('sent') });
const Input = Object.freeze({ Send: Symbol('send'), Other: Symbol('other') });
const Story = Object.freeze({ Stamp: Symbol('stamp'), Positive: Symbol('positive'), Notice: Symbol('notice'), Valid: Symbol('valid') });
class Notice { constructor(total) { this.total = total; } }
const stamp = data => { data.log.push('stamp'); };
const invoice = total => ({ total, log: [] });
function definition({ actions = [], effects = [], outputs = [], guard, postconditions = [], invariant, behaviors, capabilities, entry = [], exit = [] } = {}) {
  return new Definition({ id: 'invoices', initial: Phase.Draft,
    states: [{ id: Phase.Draft, exitActions: exit }, { id: Phase.Sent, invariant, entryActions: entry }],
    transitions: [{ id: 'send', source: Phase.Draft, event: Input.Send, target: Phase.Sent, guard, postconditions, actions, effects, outputs }],
    behaviors, capabilities, snapshot: data => data instanceof Notice ? new Notice(data.total) : structuredClone(data) });
}
const within = (made, data, body) => { const running = made.instantiate({ id: 1, data }); try { return body(running); } finally { running.dispose(); } };

// LocalAction, Effect and OutputEvent.
test('actions run in phase order and intents keep native payloads', () => {
  const named = name => ({ id: name, apply: data => { data.log.push(name); } });
  const made = definition({ exit: [named('exit')], actions: [named('a1'), named('a2')], entry: [named('entry')],
    effects: [{ id: 'mail', describe: data => new Notice(data.total) }], outputs: [{ kind: Input.Other, describe: (data, payload, facts) => new Notice(facts) }] });
  within(made, invoice(5), running => {
    const outcome = running.dispatch({ kind: Input.Send }, 9);
    assert.deepEqual(running.data.log, ['exit', 'a1', 'a2', 'entry']);
    assert.deepEqual(outcome.effects, [{ id: 'mail', payload: new Notice(5) }]);
    assert.deepEqual(outcome.outputs, [{ kind: Input.Other, payload: new Notice(9) }]);
    assert.ok(outcome.effects[0].payload instanceof Notice && Object.isFrozen(outcome.effects));
  });
});
test('functional updates replace candidate data; an action needs exactly one implementation', () => {
  within(definition({ actions: [{ id: 'double', update: data => ({ ...data, total: data.total * 2 }) }] }), invoice(4), running => {
    running.dispatch({ kind: Input.Send }); assert.equal(running.data.total, 8);
  });
  assert.throws(() => definition({ actions: [{ id: 'both', apply: stamp, update: data => data }] }), DefinitionError);
  assert.throws(() => definition({ actions: [{ id: 'neither' }] }), DefinitionError);
});
test('any failing phase rolls back the whole transition', () => {
  const failure = new Error('constructor raised');
  const raising = () => { throw failure; };
  const stamped = { id: 'stamp', apply: stamp };
  const cases = [
    [{ actions: [stamped, { id: 'boom', apply: raising }] }, CallbackError],
    [{ actions: [{ id: 'returns', apply: () => 1 }] }, CallbackError],
    [{ actions: [stamped], effects: [{ id: 'e', describe: raising }] }, CallbackError],
    [{ actions: [stamped], outputs: [{ kind: 'o', describe: raising }] }, CallbackError],
    [{ actions: [stamped], postconditions: [{ id: 'p', predicate: () => false }] }, PostconditionError],
    [{ actions: [stamped], invariant: () => false }, InvariantError],
  ];
  for (const [spec, Type] of cases) {
    within(definition(spec), invoice(1), running => {
      assert.throws(() => running.dispatch({ kind: Input.Send }), Type);
      assert.deepEqual([running.state, running.revision, running.data.log], [Phase.Draft, 0n, []]);
    });
  }
  assert.throws(() => definition({ effects: Array.from({ length: 17 }, (_, id) => ({ id, describe: () => id })) }), DefinitionLimitError);
});

// BehaviorRef, registry, roles and BoundInvariant.
const registry = (...entries) => new BehaviorRegistry({ implementations: entries.map(([story, role, callback]) => ({ reference: new BehaviorRef(story, 'v1'), role, callback })) });
test('every role resolves the exact Story and implementation', () => {
  const behaviors = registry([Story.Stamp, BehaviorRole.Action, stamp], [Story.Positive, BehaviorRole.Condition, data => data.total > 0],
    [Story.Notice, BehaviorRole.Effect, data => new Notice(data.total)], [Story.Valid, BehaviorRole.Invariant, data => data.log.includes('stamp')]);
  const made = definition({ behaviors, actions: [{ id: 'stamp', apply: new BehaviorRef(Story.Stamp, 'v1') }],
    guard: { id: 'positive', predicate: new BehaviorRef(Story.Positive, 'v1') }, effects: [{ id: 'notice', describe: new BehaviorRef(Story.Notice, 'v1') }],
    invariant: new BehaviorRef(Story.Valid, 'v1') });
  within(made, invoice(3), running => assert.deepEqual(running.dispatch({ kind: Input.Send }).effects, [{ id: 'notice', payload: new Notice(3) }]));
  within(made, invoice(0), running => assert.equal(running.dispatch({ kind: Input.Send }).kind, 'refused'));
});
test('a bound invariant rejects candidates', () => {
  const behaviors = registry([Story.Valid, BehaviorRole.Invariant, data => data.total < 0]);
  within(definition({ behaviors, invariant: new BehaviorRef(Story.Valid, 'v1') }), invoice(1), running => {
    assert.throws(() => running.dispatch({ kind: Input.Send }), error => error instanceof InvariantError && error.context.state === Phase.Sent);
  });
});
test('unresolved references are typed binding errors', () => {
  const behaviors = registry([Story.Stamp, BehaviorRole.Action, stamp]);
  assert.throws(() => definition({ behaviors, actions: [{ id: 'a', apply: new BehaviorRef(Story.Notice, 'v1') }] }),
    error => error instanceof UnknownBehaviorError && error.context.story === Story.Notice && error.context.action === 'a');
  assert.throws(() => definition({ behaviors, actions: [{ id: 'a', apply: new BehaviorRef(Story.Stamp, 'v2') }] }),
    error => error instanceof UnknownImplementationError && error.context.implementation === 'v2');
  assert.throws(() => definition({ behaviors, guard: { id: 'g', predicate: new BehaviorRef(Story.Stamp, 'v1') } }),
    error => error instanceof RoleMismatchError && error.context.expected === BehaviorRole.Condition && error.context.actual === BehaviorRole.Action && error.context.condition === 'g');
  assert.throws(() => definition({ behaviors, invariant: new BehaviorRef(Story.Stamp, 'v1') }), RoleMismatchError);
  assert.throws(() => definition({ behaviors: registry([Story.Stamp, BehaviorRole.Action, stamp], [Story.Stamp, BehaviorRole.Action, stamp]) }),
    error => error instanceof DuplicateImplementationError && error.context.story === Story.Stamp);
  assert.throws(() => definition({ behaviors, actions: [{ id: 'a', update: new BehaviorRef(Story.Stamp, 'v1') }] }), DefinitionError);
});

// MachineInstance, Event, decisions, outcomes and records.
test('outcomes and records retain native identities and revisions', () => {
  const instance = { tenant: 7 };
  const running = definition().instantiate({ id: instance, data: invoice(1) });
  assert.equal(running.id, instance);
  assert.deepEqual(running.dispatch({ kind: Input.Other }), { kind: 'refused', state: Phase.Draft, event: Input.Other });
  assert.deepEqual(running.dispatch({ kind: Input.Send }), { kind: 'transitioned', record: { transition: 'send', source: Phase.Draft,
    target: Phase.Sent, previousRevision: 0n, revision: 1n }, effects: [], outputs: [] });
  assert.deepEqual([running.state, running.revision], [Phase.Sent, 1n]);
  running.dispose();
});
test('explicit revisions and single-use decisions', () => {
  const running = definition({ actions: [{ id: 'stamp', apply: stamp }] }).instantiate({ id: 1, data: invoice(1) });
  assert.throws(() => running.dispatch({ kind: Input.Send }, undefined, { expectedRevision: 4n }),
    error => error instanceof StaleDecisionError && error.context.expectedRevision === 4n && error.context.currentRevision === 0n);
  assert.throws(() => running.dispatch({ kind: Input.Send }, undefined, { expectedRevision: 4 }), TypeError);
  const decision = running.evaluate({ kind: Input.Send }, undefined, { expectedRevision: 0n });
  assert.deepEqual([decision.expectedRevision, running.state, running.data.log], [0n, Phase.Draft, []]);
  assert.equal(running.accept(decision).kind, 'transitioned');
  assert.deepEqual(running.data.log, ['stamp']);
  assert.throws(() => running.accept(decision), DisposedError);
  const discarded = running.evaluate({ kind: Input.Other });
  discarded.dispose(); discarded[Symbol.dispose]();
  assert.throws(() => discarded.expectedRevision, DisposedError);
  running[Symbol.dispose](); running.dispose();
  for (const operation of [() => running.state, () => running.data, () => running.dispatch({ kind: Input.Send })]) assert.throws(operation, DisposedError);
});

// Capability values.
test('capability identities are checked', () => {
  const made = definition({ capabilities: [{ id: 'edit', evaluate: () => CapabilityStatus.Available }] });
  within(made, invoice(1), running => {
    assert.deepEqual(running.evaluateCapability('edit'), { capability: 'edit', instance: 1, revision: 0n, status: CapabilityStatus.Available });
    assert.throws(() => running.evaluateCapability('delete'), error => error instanceof UnknownCapabilityError && error.context.capability === 'delete');
  });
  assert.throws(() => definition({ capabilities: [{ id: 'edit', evaluate: () => CapabilityStatus.Unknown }, { id: 'edit', evaluate: () => CapabilityStatus.Unknown }] }),
    error => error instanceof DuplicateIdentifierError && error.context.reason === 'duplicate_capability' && error.context.capability === 'edit');
});

// EventOnDrop.
test('an unfinished scope delivers its event once', () => {
  within(definition(), invoice(1), running => {
    const delivered = [];
    let guard;
    // Node.js 22 has no `using` declaration; `finally` is the same scope exit.
    assert.throws(() => {
      guard = new EventOnDrop({ kind: Input.Other }, event => { delivered.push(running.dispatch(event).kind); });
      try {
        guard.event = { kind: Input.Send, payload: 'updated' };
        throw new RangeError('operation failed');
      } finally { guard[Symbol.dispose](); }
    }, RangeError);
    assert.deepEqual([delivered, running.state, guard.armed], [['transitioned'], Phase.Sent, false]);
  });
});
test('a completed scope disarms and cannot rearm', () => {
  const delivered = [];
  const guard = new EventOnDrop('abandoned', event => { delivered.push(event); });
  try { assert.equal(guard.event, 'abandoned'); guard.disarm(); guard.disarm(); } finally { guard.dispose(); }
  assert.deepEqual([delivered, guard.event], [[], undefined]);
  assert.throws(() => { guard.event = 'again'; }, DisposedError);
  new EventOnDrop('never disposed', event => { delivered.push(event); });
  assert.deepEqual(delivered, []);
});
test('handlers are synchronous and return undefined', () => {
  assert.throws(() => new EventOnDrop('x', async () => {}), TypeError);
  assert.throws(() => new EventOnDrop('x', 'not callable'), TypeError);
  assert.throws(() => new EventOnDrop('x', () => 'unhandled').dispose(), TypeError);
  assert.throws(() => new EventOnDrop('x', () => Promise.resolve()).dispose(), TypeError);
  const failure = new Error('handler raised');
  // A throwing handler replaces the scope's own exception, as any throwing `finally` does
  // (`using` would report both as a SuppressedError).
  assert.throws(() => { const guard = new EventOnDrop('x', () => { throw failure; }); try { throw new RangeError('original'); } finally { guard.dispose(); } },
    error => error === failure);
});
