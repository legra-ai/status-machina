import assert from 'node:assert/strict';
import test from 'node:test';
import { MessageChannel } from 'node:worker_threads';
import {
  BehaviorRef, BehaviorRegistry, BehaviorRole, BehaviorUse, DefinitionRecord, DuplicateIdentityError,
  DuplicateRepresentationError, IdentityMap, IdentityMapError, IdentityMapLimitError, IdentityRole, IdentityRoleError,
  MachineIdentityMaps, StateRecord, TransitionDefinitionRecord, UnknownIdentityError, UnknownRepresentationError,
} from '@legra-ai/status-machina';

const Phase = Object.freeze({ Open: Symbol('open'), Closed: Symbol('closed') });
const Close = Symbol('close');
function rejects(fn, Type, check = () => {}) {
  assert.throws(fn, error => { assert.ok(error instanceof Type, `${error.name}: ${error.message}`); check(error); return true; });
}

test('round trips return the original native values', () => {
  const states = new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'ex:Open'], [Phase.Closed, 'ex:Closed']] });
  assert.equal(states.role, 'state'); assert.equal(states.size, 2); assert.equal(states.maxEntries, 4096);
  assert.equal(states.encode(Phase.Open), 'ex:Open'); assert.equal(states.decode('ex:Closed'), Phase.Closed);
  assert.deepEqual([...states], [[Phase.Open, 'ex:Open'], [Phase.Closed, 'ex:Closed']]);
  const token = {}; const tokens = new IdentityMap({ role: IdentityRole.Definition, entries: [[token, 7n]] });
  assert.equal(tokens.decode(7n), token);
  rejects(() => tokens.encode({}), UnknownIdentityError);
});

test('identities follow Map semantics, never stringification', () => {
  const values = new IdentityMap({ role: IdentityRole.Event, entries: [[1, 'number'], ['1', 'string'], [1n, 'bigint'], [true, 'boolean']] });
  assert.deepEqual([1, '1', 1n, true].map(value => values.encode(value)), ['number', 'string', 'bigint', 'boolean']);
  const states = new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'ex:Open']] });
  for (const value of ['open', 'Symbol(open)', Symbol('open'), 'ex:Open']) {
    rejects(() => states.encode(value), UnknownIdentityError, error => assert.equal(error.context.role, IdentityRole.State));
  }
  for (const value of ['open', Phase.Open]) rejects(() => states.decode(value), UnknownRepresentationError);
});

test('duplicates name the offending entry', () => {
  rejects(() => new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'a'], [Phase.Closed, 'b'], [Phase.Open, 'c']] }),
    DuplicateIdentityError, error => { assert.equal(error.context.entry, 2); assert.equal(error.context.identity, Phase.Open); });
  rejects(() => new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'same'], [Phase.Closed, 'same']] }),
    DuplicateRepresentationError, error => assert.equal(error.context.representation, 'same'));
});

test('bounds are explicit and input is consumed lazily', () => {
  function* endless() { for (let index = 0; ; index += 1) yield [index, -index]; }
  rejects(() => new IdentityMap({ role: IdentityRole.State, entries: endless(), maxEntries: 2 }), IdentityMapLimitError, error => {
    assert.equal(error.context.limit, 2); assert.equal(error.context.entry, 2);
  });
  for (const maxEntries of [0, 4097, 1.5, -1, '4']) {
    rejects(() => new IdentityMap({ role: IdentityRole.State, entries: endless(), maxEntries }), IdentityMapLimitError);
  }
  const full = new IdentityMap({ role: IdentityRole.State, entries: Array.from({ length: 4096 }, (_, index) => [index, `s${index}`]) });
  assert.equal(full.size, 4096);
});

test('malformed entries, roles and clones fail', () => {
  rejects(() => new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open]] }), IdentityMapError, error => assert.equal(error.context.entry, 0));
  rejects(() => new IdentityMap({ role: 'states', entries: [] }), IdentityRoleError);
  rejects(() => new IdentityMap({ role: IdentityRole.State, entries: [], max: 1 }), IdentityMapError);
  const states = new IdentityMap({ role: IdentityRole.State, entries: [] });
  const { port1 } = new MessageChannel();
  assert.throws(() => port1.postMessage(states), { name: 'DataCloneError' });
  port1.close();
});

test('each role accepts only its own codec', () => {
  const states = new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'open']] });
  const events = new IdentityMap({ role: IdentityRole.Event, entries: [[Close, 'open']] });
  const maps = new MachineIdentityMaps({ state: states, event: events });
  assert.equal(maps.decode(IdentityRole.State, 'open'), Phase.Open);
  assert.equal(maps.decode(IdentityRole.Event, 'open'), Close);
  assert.equal(maps.map(IdentityRole.State), states);
  rejects(() => maps.encode(IdentityRole.Event, Phase.Open), UnknownIdentityError);
  rejects(() => new MachineIdentityMaps({ event: states }), IdentityRoleError, error => assert.equal(error.context.actual, IdentityRole.State));
  rejects(() => new MachineIdentityMaps({ states }), IdentityRoleError);
  rejects(() => maps.map(IdentityRole.Transition), IdentityRoleError);
  rejects(() => maps.decode('State', 'open'), IdentityRoleError);
});

test('external names decode into a bindable record', () => {
  const maps = new MachineIdentityMaps({
    state: new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'ex:Open'], [Phase.Closed, 'ex:Closed']] }),
    event: new IdentityMap({ role: IdentityRole.Event, entries: [[Close, 'ex:close']] }),
    transition: new IdentityMap({ role: IdentityRole.Transition, entries: [['t1', 'ex:t1']] }),
    action: new IdentityMap({ role: IdentityRole.Action, entries: [['count', 'ex:count']] }),
    story: new IdentityMap({ role: IdentityRole.Story, entries: [['count-story', 'ex:CountStory']] }),
    implementation: new IdentityMap({ role: IdentityRole.Implementation, entries: [['v1', 'ex:CountV1']] }),
  });
  const document = { initial: 'ex:Open', states: ['ex:Open', 'ex:Closed'],
    edges: [['ex:t1', 'ex:Open', 'ex:close', 'ex:Closed', 'ex:count', 'ex:CountStory', 'ex:CountV1']] };
  const decode = (role, value) => maps.decode(role, value);
  const record = new DefinitionRecord({ id: Symbol(), initial: decode(IdentityRole.State, document.initial),
    states: document.states.map(state => new StateRecord({ id: decode(IdentityRole.State, state), terminal: state === 'ex:Closed' })),
    transitions: document.edges.map(([t, a, e, b, u, story, version]) => new TransitionDefinitionRecord({
      id: decode(IdentityRole.Transition, t), source: decode(IdentityRole.State, a), event: decode(IdentityRole.Event, e),
      target: decode(IdentityRole.State, b), actions: [new BehaviorUse(decode(IdentityRole.Action, u),
        new BehaviorRef(decode(IdentityRole.Story, story), decode(IdentityRole.Implementation, version)))] })) });
  const behaviors = new BehaviorRegistry({ implementations: [{ reference: new BehaviorRef('count-story', 'v1'),
    role: BehaviorRole.Action, callback: (data, payload) => { data.push(payload); } }] });
  const machine = record.bind(behaviors).instantiate({ id: Symbol(), data: [] });
  const outcome = machine.dispatch({ kind: Close, payload: 5 });
  assert.equal(maps.encode(IdentityRole.Transition, outcome.record.transition), 'ex:t1');
  assert.equal(maps.encode(IdentityRole.State, machine.state), 'ex:Closed');
  assert.deepEqual(machine.data, [5]); machine.dispose();
});
