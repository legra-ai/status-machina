import assert from 'node:assert/strict';
import test from 'node:test';
import * as api from '@legra-ai/status-machina';
import {
  AmbiguousTransitionError, CallbackError, Definition, DefinitionError, DefinitionLimitError, DuplicateIdentifierError,
  InitialInvariantError, InternalTransitionError, InvalidConditionError, InvalidDispositionError, LimitError,
  StatusMachinaError, TerminalTransitionError, UnknownStateError, UnreachableStateError,
} from '@legra-ai/status-machina';

const Phase = Object.freeze({ Open: Symbol('open'), Review: Symbol('review'), Closed: Symbol('closed'), Ghost: Symbol('ghost') });
const Input = Object.freeze({ Submit: Symbol('submit'), Close: Symbol('close'), Ping: Symbol('ping') });
const always = () => true;
const edge = (id, source, event, target, extra = {}) => ({ id, source, event, target, ...extra });
const machine = ({ states = [Phase.Open, Phase.Closed], transitions = [], ...extra } = {}) =>
  new Definition({ id: 'd', initial: Phase.Open, states: states.map(id => ({ id })), transitions, ...extra });
function* forever(make) { for (let n = 0; ; n += 1) yield make(n); }
const rejects = (build, Type, context) => assert.throws(build, error => {
  assert.ok(error instanceof Type, `${error.name}: ${error.message}`);
  assert.ok(error instanceof DefinitionError);
  for (const [key, value] of Object.entries(context)) assert.equal(error.context[key], value, key);
  return true;
});

// MachineDefinition, builders, kinds, reachability and limits.
test('a checked definition is immutable and keeps native values', () => {
  const submit = edge('submit', Phase.Open, Input.Submit, Phase.Review);
  const definition = new Definition({ id: 'd', initial: Phase.Open,
    states: [{ id: Phase.Open }, { id: Phase.Review }, { id: Phase.Closed, terminal: true }],
    transitions: [submit, edge('close', Phase.Review, Input.Close, Phase.Closed)] });
  assert.deepEqual([definition.id, definition.initial], ['d', Phase.Open]);
  assert.ok(Object.isFrozen(definition) && Object.isFrozen(definition.states) && Object.isFrozen(definition.transitions[0]));
  assert.throws(() => { 'use strict'; definition.initial = Phase.Closed; }, TypeError);
  submit.target = Phase.Open;
  assert.equal(definition.transitions[0].target, Phase.Review);
});
test('internal transitions skip exit and entry while external self-transitions run them', () => {
  const log = [];
  const act = name => ({ id: name, apply: () => { log.push(name); } });
  const definition = new Definition({ id: 'd', initial: Phase.Open,
    states: [{ id: Phase.Open, exitActions: [act('exit')], entryActions: [act('entry')] }],
    transitions: [edge('again', Phase.Open, Input.Submit, Phase.Open, { actions: [act('again')] }),
      edge('touch', Phase.Open, Input.Ping, Phase.Open, { internal: true, actions: [act('touch')] })] });
  const running = definition.instantiate({ id: 1, data: null });
  running.dispatch({ kind: Input.Submit });
  assert.deepEqual(log.splice(0), ['exit', 'again', 'entry']);
  running.dispatch({ kind: Input.Ping });
  assert.deepEqual(log, ['touch']);
  assert.equal(running.revision, 2n); running.dispose();
});
test('graph rules raise typed errors naming native identities', () => {
  rejects(() => new Definition({ id: 'd', initial: Phase.Ghost, states: [{ id: Phase.Open }], transitions: [] }),
    UnknownStateError, { reason: 'unknown_initial_state', state: Phase.Ghost });
  rejects(() => machine({ transitions: [edge('t', Phase.Open, Input.Close, Phase.Ghost)] }),
    UnknownStateError, { reason: 'unknown_transition_state', transition: 't', state: Phase.Ghost });
  rejects(() => machine({ states: [Phase.Open], dispositions: [{ id: 'p', source: Phase.Ghost, event: Input.Ping,
    condition: { id: 'c', predicate: always }, reason: 'ignored' }] }),
  UnknownStateError, { reason: 'unknown_disposition_state', disposition: 'p', state: Phase.Ghost });
  rejects(() => machine({ states: [Phase.Open, Phase.Open] }), DuplicateIdentifierError, { reason: 'duplicate_state', state: Phase.Open });
  rejects(() => machine({ transitions: [edge('t', Phase.Open, Input.Close, Phase.Closed), edge('t', Phase.Open, Input.Submit, Phase.Closed)] }),
    DuplicateIdentifierError, { reason: 'duplicate_transition', transition: 't' });
  rejects(() => machine(), UnreachableStateError, { reason: 'unreachable_state', state: Phase.Closed });
  rejects(() => new Definition({ id: 'd', initial: Phase.Open, states: [{ id: Phase.Open, terminal: true }],
    transitions: [edge('t', Phase.Open, Input.Close, Phase.Open)] }),
  TerminalTransitionError, { reason: 'terminal_has_outgoing_transition', state: Phase.Open });
  rejects(() => machine({ transitions: [edge('t', Phase.Open, Input.Close, Phase.Closed, { internal: true })] }),
    InternalTransitionError, { reason: 'internal_transition_must_be_self', transition: 't' });
});
test('limits are kernel bounds read one entry beyond', () => {
  let consumed = 0;
  const states = forever(n => { consumed += 1; return { id: `s${n}` }; });
  rejects(() => new Definition({ id: 'd', initial: 's0', states, transitions: [], limits: { maxStates: 2 } }),
    DefinitionLimitError, { reason: 'state_limit_exceeded', limit: 2 });
  assert.equal(consumed, 3);
  rejects(() => machine({ transitions: Array.from({ length: 17 }, (_, n) => edge(n, Phase.Open, Input.Close, Phase.Closed)), limits: { maxTransitions: 16 } }),
    DefinitionLimitError, { reason: 'transition_limit_exceeded' });
  rejects(() => machine({ transitions: [edge('t', Phase.Open, Input.Close, Phase.Closed, { actions: Array.from({ length: 17 }, (_, id) => ({ id, apply: () => {} })) })] }),
    DefinitionLimitError, { reason: 'action_limit_exceeded', transition: 't' });
  for (const limits of [{ maxStates: 0 }, { maxValues: 65537 }, { maxTransitions: 1.5 }]) assert.throws(() => machine({ limits }), LimitError);
  assert.throws(() => machine({ limits: { maxRows: 1 } }), DefinitionError);
});
test('initial data is checked against the initial invariant', () => {
  const guarded = new Definition({ id: 'd', initial: Phase.Open, states: [{ id: Phase.Open, invariant: data => data > 0 }], transitions: [] });
  const accepted = guarded.instantiate({ id: 1, data: 1 }); assert.equal(accepted.data, 1); accepted.dispose();
  rejects(() => guarded.instantiate({ id: 2, data: 0 }), InitialInvariantError, { reason: 'initial_invariant_failed', state: Phase.Open });
});

// Condition expressions.
function evaluate(guard, payload) {
  const running = machine({ transitions: [edge('close', Phase.Open, Input.Close, Phase.Closed, { guard })] }).instantiate({ id: 1, data: null });
  try { return running.dispatch({ kind: Input.Close, payload }).kind; } finally { running.dispose(); }
}
test('all, any and not short-circuit in declaration order', () => {
  const calls = [];
  const leaf = (id, value) => ({ id, predicate: () => { calls.push(id); return value; } });
  assert.equal(evaluate({ id: 'g', allOf: [leaf('a', true), { id: 'any', anyOf: [leaf('b', false), leaf('c', true), leaf('never-any', true)] },
    { id: 'not', not: leaf('d', false) }] }), 'transitioned');
  assert.deepEqual(calls.splice(0), ['a', 'b', 'c', 'd']);
  assert.equal(evaluate({ id: 'g', allOf: [leaf('a', false), leaf('never-all', true)] }), 'refused');
  assert.deepEqual(calls, ['a']);
});
test('nesting and operand bounds are the kernel limits', () => {
  const nested = depth => { let condition = { id: 'leaf', predicate: always }; for (let level = 0; level < depth - 1; level += 1) condition = { id: level, not: condition }; return condition; };
  assert.equal(evaluate(nested(32)), 'refused');
  for (const depth of [33, 200]) assert.throws(() => evaluate(nested(depth)), error => error instanceof DefinitionLimitError && error.context.reason === 'depth_limit_exceeded');
  assert.throws(() => evaluate({ id: 'wide', allOf: forever(id => ({ id, predicate: always })) }),
    error => error instanceof DefinitionLimitError && error.context.reason === 'operand_limit_exceeded' && error.context.condition === 'wide');
  assert.throws(() => evaluate({ id: 'empty', anyOf: [] }), error => error instanceof InvalidConditionError && error.context.reason === 'empty_group' && error.context.condition === 'empty');
  assert.throws(() => evaluate({ id: 'both', predicate: always, not: { id: 'x', predicate: always } }), DefinitionError);
});
test('predicate exceptions and non-booleans are callback errors, not false', () => {
  const failure = new Error('predicate raised'); const later = [];
  assert.throws(() => evaluate({ id: 'g', anyOf: [{ id: 'x', predicate: () => { throw failure; } }, { id: 'y', predicate: () => { later.push(1); return true; } }] }),
    error => error instanceof CallbackError && error.cause === failure);
  assert.deepEqual(later, []);
  assert.throws(() => evaluate({ id: 'g', predicate: () => 1 }), error => error instanceof CallbackError && error.cause instanceof TypeError);
});

// EventDisposition.
const disposed = ({ predicate = always, reason = 'duplicate ping', extra = [] } = {}) => machine({
  transitions: [edge('close', Phase.Open, Input.Close, Phase.Closed), ...extra],
  dispositions: [{ id: 'ping', source: Phase.Open, event: Input.Ping, condition: { id: 'harmless', predicate }, reason }] });
test('ignored is distinct from refused and accepts nothing', () => {
  const running = disposed({ predicate: (data, payload) => payload === 'dup' }).instantiate({ id: 1, data: [1] });
  assert.deepEqual(running.dispatch({ kind: Input.Ping, payload: 'dup' }), { kind: 'ignored', disposition: 'ping', state: Phase.Open, event: Input.Ping, reason: 'duplicate ping' });
  assert.deepEqual([running.state, running.revision, running.data], [Phase.Open, 0n, [1]]);
  assert.deepEqual(running.dispatch({ kind: Input.Ping, payload: 'new' }), { kind: 'refused', state: Phase.Open, event: Input.Ping });
  assert.deepEqual(running.dispatch({ kind: Input.Submit }), { kind: 'refused', state: Phase.Open, event: Input.Submit });
  running.dispose();
});
test('disposition reasons, overlaps and identities are checked', () => {
  for (const reason of ['', '   ', 'x'.repeat(257)]) rejects(() => disposed({ reason }), InvalidDispositionError, { reason: 'invalid_reason', disposition: 'ping' });
  const running = disposed({ extra: [edge('ping', Phase.Open, Input.Ping, Phase.Closed)] }).instantiate({ id: 1, data: null });
  assert.throws(() => running.dispatch({ kind: Input.Ping }), error => error instanceof AmbiguousTransitionError && error.context.state === Phase.Open);
  running.dispose();
  rejects(() => machine({ transitions: [edge('close', Phase.Open, Input.Close, Phase.Closed)], dispositions: [
    { id: 'p', source: Phase.Open, event: Input.Ping, condition: { id: 'c', predicate: always }, reason: 'a' },
    { id: 'p', source: Phase.Open, event: Input.Submit, condition: { id: 'c2', predicate: always }, reason: 'b' }] }),
  DuplicateIdentifierError, { reason: 'duplicate_disposition', disposition: 'p' });
});

// Error taxonomy.
test('every exported error class is a StatusMachinaError', () => {
  for (const [name, value] of Object.entries(api)) {
    if (typeof value === 'function' && value.prototype instanceof Error) assert.ok(value === StatusMachinaError || value.prototype instanceof StatusMachinaError, name);
  }
  assert.ok(api.BindingError.prototype instanceof DefinitionError);
});
