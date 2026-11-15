import assert from 'node:assert/strict';
import test from 'node:test';
import {
  AllRecord, AnyRecord, BehaviorRef, BehaviorRegistry, BehaviorRole, BehaviorUse, BindingError, CallbackError,
  ConditionRecordError, DefinitionError, DefinitionRecord, DispositionRecord, DispositionRecordError, LimitError,
  NotRecord, PredicateRecord, RecordError, RoleMismatchError, StateRecord, TransitionDefinitionRecord,
  TransitionRecordError, UnknownBehaviorError, UnknownImplementationError,
} from '../index.mjs';

const Phase = Object.freeze({ Draft: Symbol('draft'), Sent: Symbol('sent'), Paid: Symbol('paid') });
const Input = Object.freeze({ Send: Symbol('send'), Pay: Symbol('pay'), Nudge: Symbol('nudge'), SentNotice: Symbol('sent notice') });
const Story = Object.freeze({ HasLines: Symbol('has lines'), Stamp: Symbol('stamp'), Notify: Symbol('notify'),
  Announce: Symbol('announce'), Always: Symbol('always') });
const V1 = Symbol('v1'); const V2 = Symbol('v2');
const Edge = Object.freeze({ Send: Symbol('send edge'), Pay: Symbol('pay edge') });
const Use = Object.freeze({ Guard: Symbol('guard'), Stamp: Symbol('stamp use'), Notify: Symbol('notify use'), Ignore: Symbol('ignore') });
const HAS_LINES = new BehaviorRef(Story.HasLines, V1); const STAMP = new BehaviorRef(Story.Stamp, V1);
const NOTIFY = new BehaviorRef(Story.Notify, V1); const ANNOUNCE = new BehaviorRef(Story.Announce, V1);
const ALWAYS = new BehaviorRef(Story.Always, V1);
const stamp = data => { data.stamps += 1; };

function registry(...extra) {
  return new BehaviorRegistry({ implementations: [
    { reference: HAS_LINES, role: BehaviorRole.Condition, callback: data => data.lines > 0 },
    { reference: STAMP, role: BehaviorRole.Action, callback: stamp },
    { reference: NOTIFY, role: BehaviorRole.Effect, callback: data => ['notify', data.lines] },
    { reference: ANNOUNCE, role: BehaviorRole.Output, callback: data => data.stamps },
    { reference: ALWAYS, role: BehaviorRole.Condition, callback: () => true },
    ...extra] });
}
function record({ guard = new PredicateRecord(new BehaviorUse(Use.Guard, HAS_LINES)), states, transitions, dispositions = [] } = {}) {
  return new DefinitionRecord({ id: Symbol('invoice'), initial: Phase.Draft, dispositions,
    states: states ?? [new StateRecord({ id: Phase.Draft }), new StateRecord({ id: Phase.Sent }), new StateRecord({ id: Phase.Paid, terminal: true })],
    transitions: transitions ?? [
      new TransitionDefinitionRecord({ id: Edge.Send, source: Phase.Draft, event: Input.Send, target: Phase.Sent, guard,
        actions: [new BehaviorUse(Use.Stamp, STAMP)], effects: [new BehaviorUse(Use.Notify, NOTIFY)],
        outputs: [new BehaviorUse(Input.SentNotice, ANNOUNCE)] }),
      new TransitionDefinitionRecord({ id: Edge.Pay, source: Phase.Sent, event: Input.Pay, target: Phase.Paid }),
    ] });
}
function rejects(fn, Type, check = () => {}) {
  assert.throws(fn, error => { assert.ok(error instanceof Type, `${error.name}: ${error.message}`); check(error); return true; });
}

test('a bound record runs registered implementations with native identities', () => {
  const nudge = new DispositionRecord({ id: Use.Ignore, source: Phase.Sent, event: Input.Nudge,
    condition: new PredicateRecord(new BehaviorUse(Use.Ignore, ALWAYS)), reason: 'reminders are harmless' });
  const source = record({ dispositions: [nudge] });
  const definition = source.bind(registry());
  assert.equal(definition.record, source); assert.equal(definition.initial, Phase.Draft);
  assert.deepEqual(definition.states.map(state => state.id), Object.values(Phase));
  const machine = definition.instantiate({ id: Symbol(), data: { lines: 2, stamps: 0 } });
  const outcome = machine.dispatch({ kind: Input.Send, payload: undefined });
  assert.equal(outcome.kind, 'transitioned'); assert.equal(outcome.record.transition, Edge.Send);
  assert.equal(outcome.effects[0].id, Use.Notify); assert.deepEqual(outcome.effects[0].payload, ['notify', 2]);
  assert.equal(outcome.outputs[0].kind, Input.SentNotice); assert.equal(outcome.outputs[0].payload, 1);
  assert.deepEqual(machine.data, { lines: 2, stamps: 1 });
  const ignored = machine.dispatch({ kind: Input.Nudge, payload: undefined });
  assert.deepEqual([ignored.kind, ignored.disposition, ignored.reason], ['ignored', Use.Ignore, 'reminders are harmless']);
  assert.equal(machine.dispatch({ kind: Input.Pay, payload: undefined }).record.target, Phase.Paid);
  machine.dispose();
});

test('condition records short-circuit in declared order', () => {
  const calls = []; const never = new BehaviorRef(Story.Always, V2);
  const behaviors = registry({ reference: never, role: BehaviorRole.Condition, callback: () => { calls.push('never'); return false; } });
  const any = new AnyRecord(Symbol(), [new PredicateRecord(new BehaviorUse(Symbol(), ALWAYS)), new PredicateRecord(new BehaviorUse(Symbol(), never))]);
  const guard = new AllRecord(Symbol(), [new NotRecord(Symbol(), new NotRecord(Symbol(), any))]);
  assert.equal(guard.kind, 'all'); assert.ok(Object.isFrozen(guard.operands));
  const machine = record({ guard }).bind(behaviors).instantiate({ id: Symbol(), data: { lines: 1, stamps: 0 } });
  assert.equal(machine.dispatch({ kind: Input.Send, payload: undefined }).kind, 'transitioned');
  assert.deepEqual(calls, []); machine.dispose();
});

test('a bound callback failure keeps its original cause', () => {
  const failure = new Error('ledger unavailable'); const flaky = new BehaviorRef(Story.HasLines, V2);
  const behaviors = registry({ reference: flaky, role: BehaviorRole.Condition, callback: () => { throw failure; } });
  const machine = record({ guard: new PredicateRecord(new BehaviorUse(Use.Guard, flaky)) }).bind(behaviors)
    .instantiate({ id: Symbol(), data: { lines: 1, stamps: 0 } });
  rejects(() => machine.dispatch({ kind: Input.Send, payload: undefined }), CallbackError, error => assert.equal(error.cause, failure));
  assert.equal(machine.state, Phase.Draft); machine.dispose();
});

test('records reject functions, plain objects and unknown fields at construction', () => {
  rejects(() => new BehaviorUse(Use.Stamp, stamp), RecordError);
  rejects(() => new BehaviorUse(Use.Stamp, { story: Story.Stamp, implementation: V1 }), RecordError);
  rejects(() => new StateRecord({ id: Phase.Draft, invariant: () => true }), RecordError);
  rejects(() => new StateRecord({ id: Phase.Draft, entryActions: [STAMP] }), RecordError);
  rejects(() => new StateRecord({ id: Phase.Draft, terminal: 'yes' }), RecordError);
  rejects(() => new StateRecord({ id: Phase.Draft, onEnter: [] }), RecordError);
  rejects(() => new TransitionDefinitionRecord({ id: Edge.Send, source: Phase.Draft, event: Input.Send }), RecordError);
  rejects(() => new AllRecord(Symbol(), [HAS_LINES]), RecordError);
  rejects(() => new PredicateRecord(HAS_LINES), RecordError);
  rejects(() => new DefinitionRecord({ id: Symbol(), initial: Phase.Draft, states: [{ id: Phase.Draft }], transitions: [] }), RecordError);
  rejects(() => new AnyRecord(Symbol(), Array(17).fill(new PredicateRecord(new BehaviorUse(Symbol(), ALWAYS)))), LimitError);
  rejects(() => new DefinitionRecord({ id: Symbol(), initial: Phase.Draft, states: [], transitions: [], maxStates: 0 }), LimitError);
  rejects(() => record().bind({ implementations: [] }), DefinitionError);
  rejects(() => record().bind(registry(), { limits: {} }), RecordError);
});

test('Story resolution never substitutes or loads code', () => {
  const missing = new BehaviorRef('node:child_process', V1);
  rejects(() => record({ guard: new PredicateRecord(new BehaviorUse(Use.Guard, missing)) }).bind(registry()), UnknownBehaviorError, error => {
    assert.equal(error.context.story, 'node:child_process'); assert.ok(error instanceof BindingError);
  });
  const other = new BehaviorRef(Story.HasLines, V2);
  rejects(() => record({ guard: new PredicateRecord(new BehaviorUse(Use.Guard, other)) }).bind(registry()), UnknownImplementationError, error => {
    assert.equal(error.context.story, Story.HasLines); assert.equal(error.context.implementation, V2);
  });
  rejects(() => record({ guard: new PredicateRecord(new BehaviorUse(Use.Guard, STAMP)) }).bind(registry()), RoleMismatchError, error => {
    assert.equal(error.context.expected, BehaviorRole.Condition); assert.equal(error.context.actual, BehaviorRole.Action);
  });
});

test('structural failures name the native record', () => {
  let deep = new PredicateRecord(new BehaviorUse('innermost', ALWAYS));
  for (let depth = 0; depth < 32; depth += 1) deep = new NotRecord(depth, deep);
  rejects(() => record({ guard: deep }).bind(registry()), ConditionRecordError, error => assert.equal(error.context.condition, 'innermost'));
  rejects(() => record({ guard: new AllRecord('empty', []) }).bind(registry()), ConditionRecordError, error => assert.equal(error.context.condition, 'empty'));
  const internal = [new TransitionDefinitionRecord({ id: Edge.Send, source: Phase.Draft, event: Input.Send, target: Phase.Sent, internal: true }),
    new TransitionDefinitionRecord({ id: Edge.Pay, source: Phase.Sent, event: Input.Pay, target: Phase.Paid })];
  rejects(() => record({ transitions: internal }).bind(registry()), TransitionRecordError, error => assert.equal(error.context.transition, Edge.Send));
  const noisy = new DispositionRecord({ id: Use.Ignore, source: Phase.Sent, event: Input.Nudge,
    condition: new PredicateRecord(new BehaviorUse(Use.Ignore, ALWAYS)), reason: 'x'.repeat(257) });
  rejects(() => record({ dispositions: [noisy] }).bind(registry()), DispositionRecordError, error => assert.equal(error.context.disposition, Use.Ignore));
});

test('graph rules report native identities', () => {
  const states = [new StateRecord({ id: Phase.Draft }), new StateRecord({ id: Phase.Sent }), new StateRecord({ id: Phase.Paid, terminal: true })];
  const onlySend = [new TransitionDefinitionRecord({ id: Edge.Send, source: Phase.Draft, event: Input.Send, target: Phase.Sent })];
  rejects(() => record({ states, transitions: onlySend }).bind(registry()), RecordError, error => assert.equal(error.context.state, Phase.Paid));
  const dangling = [new TransitionDefinitionRecord({ id: Edge.Send, source: Phase.Draft, event: Input.Send, target: Phase.Paid })];
  rejects(() => record({ states: [new StateRecord({ id: Phase.Draft })], transitions: dangling }).bind(registry()), RecordError, error => {
    assert.equal(error.context.transition, Edge.Send); assert.equal(error.context.state, Phase.Paid);
  });
});
