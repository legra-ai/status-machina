'use strict';
const { DefinitionError } = require('./errors.cjs');
const { bounded } = require('./compile.cjs');
function fields(value, allowed, label, required = []) {
  if (value === null || typeof value !== 'object') throw new DefinitionError(`${label} must be a specification object`);
  for (const key of Reflect.ownKeys(value)) {
    if (!allowed.includes(key)) throw new DefinitionError(`Unknown ${label} field ${String(key)}`);
  }
  for (const key of required) if (!Object.hasOwn(value, key)) throw new DefinitionError(`${label} requires ${key}`);
}
function action(value) {
  fields(value, ['id', 'apply', 'update'], 'action', ['id']);
  return Object.freeze({ ...value });
}
function intent(value, output = false) {
  fields(value, [output ? 'kind' : 'id', 'describe'], output ? 'output' : 'effect', [output ? 'kind' : 'id', 'describe']);
  return Object.freeze({ ...value });
}
function list(values, label, convert) {
  return Object.freeze(bounded(values === undefined ? [] : values, 16, label).map(convert));
}
function condition(value, depth = 1) {
  if (depth > 32) throw new DefinitionError('Condition nesting exceeds 32');
  fields(value, ['id', 'predicate', 'allOf', 'anyOf', 'not'], 'condition', ['id']);
  const copy = { ...value };
  for (const name of ['allOf', 'anyOf']) {
    if (value[name] !== undefined) copy[name] = list(value[name], 'condition operands', child => condition(child, depth + 1));
  }
  if (value.not !== undefined) copy.not = condition(value.not, depth + 1);
  return Object.freeze(copy);
}
function state(value) {
  fields(value, ['id', 'terminal', 'invariant', 'entryActions', 'exitActions', 'entryEffects', 'exitEffects'], 'state', ['id']);
  return Object.freeze({ ...value, entryActions: list(value.entryActions, 'entry actions', action),
    exitActions: list(value.exitActions, 'exit actions', action),
    entryEffects: list(value.entryEffects, 'entry effects', item => intent(item)),
    exitEffects: list(value.exitEffects, 'exit effects', item => intent(item)) });
}
function transition(value) {
  fields(value, ['id', 'source', 'event', 'target', 'internal', 'guard', 'postconditions', 'actions', 'effects', 'outputs'],
    'transition', ['id', 'source', 'event', 'target']);
  return Object.freeze({ ...value, guard: value.guard === undefined ? undefined : condition(value.guard),
    postconditions: list(value.postconditions, 'postconditions', item => condition(item)),
    actions: list(value.actions, 'actions', action), effects: list(value.effects, 'effects', item => intent(item)),
    outputs: list(value.outputs, 'outputs', item => intent(item, true)) });
}
function disposition(value) {
  fields(value, ['id', 'source', 'event', 'condition', 'reason'], 'disposition', ['id', 'source', 'event', 'condition', 'reason']);
  return Object.freeze({ ...value, condition: condition(value.condition) });
}
module.exports = { fields, state, transition, disposition };
