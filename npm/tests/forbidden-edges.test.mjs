import assert from 'node:assert/strict';
import test from 'node:test';
import {
  BehaviorRef, BehaviorRegistry, BehaviorRole, BehaviorUse, Definition, DefinitionError, DefinitionLimitError,
  DefinitionRecord, DuplicateIdentifierError, ForbiddenEdge, ForbiddenTransitionError, InspectionError, LimitError,
  PredicateRecord, RecordError, StateRecord, TerminalTransitionError, TransitionDefinitionRecord, UnknownStateError,
} from '@legra-ai/status-machina';

const Session = Object.freeze({ Open: Symbol('open'), Sealing: Symbol('sealing'), Done: Symbol('done'),
  Failed: Symbol('failed'), Cancelled: Symbol('cancelled'), Ghost: Symbol('ghost') });
const Signal = Object.freeze({ Seal: Symbol('seal'), Finish: Symbol('finish'), Fail: Symbol('fail'),
  Cancel: Symbol('cancel'), Retry: Symbol('retry'), Stale: Symbol('stale') });
const TERMINAL = [Session.Done, Session.Failed, Session.Cancelled];
const STATES = [Session.Open, Session.Sealing, ...TERMINAL];
const SEALING_CANCELLED = new ForbiddenEdge(Session.Sealing, Session.Cancelled);
const NEVER = new BehaviorRef(Symbol('never'), 1);
const REGISTRY = new BehaviorRegistry({ implementations: [{ reference: NEVER, role: BehaviorRole.Condition, callback: () => false }] });
const LEGAL = [
  { id: 'seal', source: Session.Open, event: Signal.Seal, target: Session.Sealing },
  { id: 'finish', source: Session.Sealing, event: Signal.Finish, target: Session.Done },
  { id: 'fail', source: Session.Sealing, event: Signal.Fail, target: Session.Failed },
  { id: 'cancel', source: Session.Open, event: Signal.Cancel, target: Session.Cancelled },
];
function session({ extra = [], forbidden = [SEALING_CANCELLED], first = false, ...options } = {}) {
  return new Definition({ id: 'session', initial: Session.Open, behaviors: REGISTRY, forbiddenEdges: forbidden,
    states: STATES.map(id => ({ id, terminal: TERMINAL.includes(id) })),
    transitions: first ? [...extra, ...LEGAL] : [...LEGAL, ...extra], ...options });
}
function record({ extra = [], forbidden = [SEALING_CANCELLED], first = false } = {}) {
  const legal = LEGAL.map(edge => new TransitionDefinitionRecord(edge));
  return new DefinitionRecord({ id: 'session', initial: Session.Open, forbiddenEdges: forbidden,
    states: STATES.map(id => new StateRecord({ id, terminal: TERMINAL.includes(id) })),
    transitions: first ? [...extra, ...legal] : [...legal, ...extra] });
}
function failure(build) {
  try { build(); } catch (error) { if (error instanceof DefinitionError) return error; throw error; }
  assert.fail('the definition was accepted');
}
const CONFLICT = { reason: 'forbidden_transition', transition: 'abandon', source: Session.Sealing, target: Session.Cancelled };
function pick(context, keys) { return Object.fromEntries(keys.map(key => [key, context[key]])); }

test('Sealing -> Cancelled is rejected in either order, for every event and a false guard', () => {
  for (const event of Object.values(Signal)) {
    for (const guard of [undefined, { id: 'never', predicate: () => false }, { id: 'never', predicate: NEVER }]) {
      const abandon = { id: 'abandon', source: Session.Sealing, event, target: Session.Cancelled, ...(guard ? { guard } : {}) };
      for (const first of [false, true]) {
        const error = failure(() => session({ extra: [abandon], first }));
        assert.ok(error instanceof ForbiddenTransitionError);
        assert.deepEqual(pick(error.context, Object.keys(CONFLICT)), CONFLICT);
      }
    }
  }
});

test('records reject the same conflict with identical context, in either order', () => {
  const guard = new PredicateRecord(new BehaviorUse('never', NEVER));
  for (const event of Object.values(Signal)) {
    for (const guarded of [false, true]) {
      const native = failure(() => session({ extra: [{ id: 'abandon', source: Session.Sealing, event, target: Session.Cancelled,
        ...(guarded ? { guard: { id: 'never', predicate: NEVER } } : {}) }] }));
      const abandon = new TransitionDefinitionRecord({ id: 'abandon', source: Session.Sealing, event, target: Session.Cancelled,
        ...(guarded ? { guard } : {}) });
      const other = new ForbiddenEdge(Session.Open, Session.Done);
      for (const first of [false, true]) {
        for (const forbidden of [[SEALING_CANCELLED, other], [other, SEALING_CANCELLED]]) {
          const bound = failure(() => record({ extra: [abandon], forbidden, first }).bind(REGISTRY));
          assert.equal(bound.constructor, RecordError);
          assert.ok(native instanceof ForbiddenTransitionError);
          assert.deepEqual(bound.context, native.context);
        }
      }
    }
  }
});

test('unknown endpoints, duplicates and forbidden self-transitions are typed', () => {
  const self = new ForbiddenEdge(Session.Sealing, Session.Sealing);
  const retry = { id: 'retry', source: Session.Sealing, event: Signal.Retry, target: Session.Sealing };
  const cases = [
    [() => session({ forbidden: [new ForbiddenEdge(Session.Ghost, Session.Cancelled)] }), UnknownStateError,
      { reason: 'unknown_forbidden_edge_state', source: Session.Ghost, target: Session.Cancelled, state: Session.Ghost }],
    [() => session({ forbidden: [new ForbiddenEdge(Session.Sealing, Session.Ghost)] }), UnknownStateError,
      { reason: 'unknown_forbidden_edge_state', source: Session.Sealing, target: Session.Ghost, state: Session.Ghost }],
    [() => session({ forbidden: [SEALING_CANCELLED, SEALING_CANCELLED] }), DuplicateIdentifierError,
      { reason: 'duplicate_forbidden_edge', source: Session.Sealing, target: Session.Cancelled }],
    [() => session({ extra: [retry], forbidden: [self] }), ForbiddenTransitionError,
      { reason: 'forbidden_transition', transition: 'retry', source: Session.Sealing, target: Session.Sealing }],
    [() => session({ extra: [{ ...retry, internal: true }], forbidden: [self] }), ForbiddenTransitionError,
      { reason: 'forbidden_transition', transition: 'retry', source: Session.Sealing, target: Session.Sealing }],
  ];
  for (const [build, Type, context] of cases) {
    const error = failure(build);
    assert.ok(error instanceof Type, context.reason);
    assert.deepEqual(pick(error.context, Object.keys(context)), context);
  }
});

test('record binding matches every native validation outcome', () => {
  const retry = { id: 'retry', source: Session.Sealing, event: Signal.Retry, target: Session.Sealing };
  const reopen = { id: 'reopen', source: Session.Done, event: Signal.Retry, target: Session.Sealing };
  const cases = [[[], [new ForbiddenEdge(Session.Ghost, Session.Cancelled)]], [[], [SEALING_CANCELLED, SEALING_CANCELLED]],
    [[retry], [new ForbiddenEdge(Session.Sealing, Session.Sealing)]], [[reopen], [new ForbiddenEdge(Session.Done, Session.Sealing)]],
    [[reopen], []]];
  for (const [extra, forbidden] of cases) {
    const native = failure(() => session({ extra, forbidden }));
    const bound = failure(() => record({ extra: extra.map(edge => new TransitionDefinitionRecord(edge)), forbidden }).bind(REGISTRY));
    assert.equal(bound.constructor, RecordError);
    assert.deepEqual(bound.context, native.context);
  }
});

test('terminal states keep rejecting every outgoing edge', () => {
  const reopen = { id: 'reopen', source: Session.Done, event: Signal.Retry, target: Session.Sealing };
  for (const forbidden of [[], [new ForbiddenEdge(Session.Done, Session.Sealing)]]) {
    const error = failure(() => session({ extra: [reopen], forbidden }));
    assert.ok(error instanceof TerminalTransitionError);
    assert.equal(error.context.state, Session.Done);
  }
  const redundant = new ForbiddenEdge(Session.Done, Session.Open);
  assert.deepEqual(session({ forbidden: [redundant] }).forbiddenEdges, [redundant]);
});

test('the bound is checked reading one prohibition beyond it', () => {
  let consumed = 0;
  function* endless() { for (;;) { consumed += 1; yield new ForbiddenEdge(Session.Open, Session.Done); } }
  const error = failure(() => session({ forbidden: endless() }));
  assert.ok(error instanceof DefinitionLimitError);
  assert.deepEqual(pick(error.context, ['reason', 'limit']), { reason: 'forbidden_edge_limit_exceeded', limit: 128 });
  assert.equal(consumed, 129);
  assert.throws(() => record({ forbidden: Array(129).fill(SEALING_CANCELLED) }), LimitError);
});

test('construction runs no callback and accepts only ForbiddenEdge values', () => {
  const calls = [];
  const abandon = { id: 'abandon', source: Session.Sealing, event: Signal.Cancel, target: Session.Cancelled,
    guard: { id: 'g', predicate: () => { calls.push('guard'); return true; } },
    actions: [{ id: 'a', apply: () => { calls.push('action'); } }] };
  assert.throws(() => session({ extra: [abandon] }), ForbiddenTransitionError);
  assert.deepEqual(calls, []);
  assert.throws(() => session({ forbidden: [{ source: Session.Sealing, target: Session.Cancelled }] }), DefinitionError);
  assert.throws(() => record({ forbidden: [{ source: Session.Sealing, target: Session.Cancelled }] }), RecordError);
  assert.ok(Object.isFrozen(SEALING_CANCELLED));
});

test('dispatch keeps default refusal, legal edges and dispositions', () => {
  const definition = session({ dispositions: [{ id: 'stale', source: Session.Sealing, event: Signal.Stale,
    condition: { id: 'always', predicate: () => true }, reason: 'stale completion' }] });
  for (const [signal, target] of [[Signal.Finish, Session.Done], [Signal.Fail, Session.Failed]]) {
    const machine = definition.instantiate({ id: 1, data: { n: 1 } });
    assert.equal(machine.dispatch({ kind: Signal.Seal, payload: null }).kind, 'transitioned');
    assert.deepEqual(machine.dispatch({ kind: Signal.Cancel, payload: null }), { kind: 'refused', state: Session.Sealing, event: Signal.Cancel });
    assert.deepEqual([machine.state, machine.revision, machine.data], [Session.Sealing, 1n, { n: 1 }]);
    assert.deepEqual(machine.dispatch({ kind: Signal.Stale, payload: null }),
      { kind: 'ignored', disposition: 'stale', state: Session.Sealing, event: Signal.Stale, reason: 'stale completion' });
    assert.equal(machine.revision, 1n);
    assert.equal(machine.dispatch({ kind: signal, payload: null }).kind, 'transitioned');
    assert.equal(machine.state, target);
    machine.dispose();
  }
  const bound = record().bind(REGISTRY);
  assert.deepEqual(bound.forbiddenEdges, [SEALING_CANCELLED]);
  assert.deepEqual(bound.record.forbiddenEdges, [SEALING_CANCELLED]);
});

test('coverage lists typed pairs independent of declaration order; diagrams never draw them', () => {
  const pairs = [SEALING_CANCELLED, new ForbiddenEdge(Session.Open, Session.Done), new ForbiddenEdge(Session.Sealing, Session.Sealing)];
  const forward = session({ forbidden: pairs }).structuralCoverage();
  const reversed = session({ forbidden: [...pairs].reverse() }).structuralCoverage();
  assert.deepEqual(forward, reversed);
  assert.deepEqual(forward.forbiddenEdges.map(edge => [edge.source, edge.target]),
    [[Session.Open, Session.Done], [Session.Sealing, Session.Sealing], [Session.Sealing, Session.Cancelled]]);
  assert.ok(forward.forbiddenEdges.every(edge => edge instanceof ForbiddenEdge));
  const plain = session({ forbidden: [] }).structuralCoverage();
  assert.deepEqual([forward.states, forward.transitions], [plain.states, plain.transitions]);
  assert.deepEqual(plain.forbiddenEdges, []);
  assert.equal(session({ forbidden: pairs }).structuralCoverage({ maxEntries: 12 }).forbiddenEdges.length, 3);
  assert.throws(() => session({ forbidden: pairs }).structuralCoverage({ maxEntries: 11 }),
    error => error instanceof InspectionError && error.context.limit === 11);
  const labels = { states: new Map(STATES.map(id => [id, id.description])),
    transitions: new Map(LEGAL.map(edge => [edge.id, edge.id])),
    events: new Map(Object.values(Signal).map(id => [id, id.description])) };
  const plainDiagram = session({ forbidden: [] }).diagram(labels);
  assert.equal(session({ forbidden: pairs }).diagram(labels), plainDiagram);
  assert.equal(session({ forbidden: pairs }).diagram(labels, { maxEntries: 9 }), plainDiagram);
});
