import assert from 'node:assert/strict';
import test from 'node:test';
import { Definition, InspectionError } from '../index.mjs';
const ready = Symbol('same'); const done = Symbol('same');
const finish = Symbol('finish'); const duplicate = Symbol('duplicate');
const edge = Symbol('edge'); const guard = Symbol('guard'); const rule = Symbol('rule');
const forbidden = () => { throw new Error('inspection evaluated behavior'); };
function definition() {
  return new Definition({ id: Symbol(), initial: ready,
    states: [{ id: ready, invariant: forbidden }, { id: done, terminal: true }],
    transitions: [{ id: edge, source: ready, event: finish, target: done, guard: { id: guard, predicate: forbidden } }],
    dispositions: [{ id: rule, source: done, event: duplicate, condition: { id: guard, predicate: forbidden }, reason: 'Already done' }] });
}
const labels = { states: new Map([[ready, 'Same "label"'], [done, 'Same "label"']]),
  transitions: new Map([[edge, 'Finish <now>']]), events: new Map([[finish, 'Finish'], [duplicate, 'Duplicate']]),
  conditions: new Map([[guard, 'Allowed?']]), dispositions: new Map([[rule, 'Harmless']]) };
test('coverage preserves symbols and reports structure without executing callbacks', () => {
  const coverage = definition().structuralCoverage();
  assert.deepEqual(coverage.states.map(row => row.id), [ready, done]);
  assert.deepEqual(coverage.states[0], { id: ready, initial: true, terminal: false, incoming: 0, outgoing: 1, dispositions: 0 });
  assert.deepEqual(coverage.states[1], { id: done, initial: false, terminal: true, incoming: 1, outgoing: 0, dispositions: 1 });
  assert.deepEqual(coverage.transitions, [edge]); assert.deepEqual(coverage.dispositions, [rule]);
  assert.ok(Object.isFrozen(coverage)); assert.ok(Object.isFrozen(coverage.states)); assert.ok(Object.isFrozen(coverage.states[0]));
});
test('Mermaid escapes labels and keeps two equal labels as distinct state nodes', () => {
  const graph = definition(); const diagram = graph.diagram(labels);
  assert.ok(diagram.startsWith('stateDiagram-v2\n'));
  assert.equal(diagram.match(/state "Same #34;label#34;"/g).length, 2);
  assert.ok(diagram.includes('Finish #60;now#62; / Finish [guard: Allowed#63;]'));
  assert.ok(diagram.includes('disposition: Harmless / Duplicate')); assert.ok(diagram.includes('s0 --> s1'));
  assert.equal(diagram, graph.diagram(labels));
});
test('entry/output limits and missing labels raise explicit inspection errors', () => {
  const graph = definition();
  assert.throws(() => graph.structuralCoverage({ maxEntries: 3 }), error => error instanceof InspectionError && error.context.limit === 3);
  assert.throws(() => graph.diagram(labels, { maxOutputBytes: 10 }), InspectionError);
  assert.throws(() => graph.diagram({}), error => error instanceof InspectionError && error.context.identifier === ready);
  assert.throws(() => graph.structuralCoverage({ maxEntries: 0 }), InspectionError);
  assert.throws(() => graph.structuralCoverage({ maxOutputBytes: true }), InspectionError);
  assert.throws(() => graph.structuralCoverage({ typo: 10 }), InspectionError);
});
test('invalid labels never stringify objects and UTF-8 budgets count bytes', () => {
  const graph = definition();
  assert.throws(() => graph.diagram({ states: new Map([[ready, { toString: forbidden }]]) }), InspectionError);
  assert.throws(() => graph.diagram({ states: new Map([[ready, '\ud800']]) }), InspectionError);
  assert.throws(() => graph.diagram({ states: new Map([[ready, '界'.repeat(10)]]) }, { maxOutputBytes: 20 }), InspectionError);
});
test('an invalid provided label Map fails even when that role has no rendered identities', () => {
  const graph = new Definition({ id: Symbol(), initial: ready, states: [{ id: ready }], transitions: [] });
  assert.throws(() => graph.diagram({ states: new Map([[ready, 'Ready']]), events: 42 }), InspectionError);
});
test('state reports and diagram indices follow declaration order rather than initial first', () => {
  const graph = new Definition({ id: Symbol(), initial: ready, states: [{ id: done, terminal: true }, { id: ready }],
    transitions: [{ id: edge, source: ready, event: finish, target: done }] });
  assert.deepEqual(graph.structuralCoverage().states.map(row => row.id), [done, ready]);
  const diagram = graph.diagram({ states: new Map([[done, 'Done'], [ready, 'Ready']]),
    transitions: new Map([[edge, 'Finish']]), events: new Map([[finish, 'Finish']]) });
  assert.ok(diagram.includes('state "Done" as s0')); assert.ok(diagram.includes('[*] --> s1'));
});
