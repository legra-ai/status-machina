'use strict';
const { DefinitionError, LimitError } = require('./errors.cjs');
const UNKNOWN_EVENT = 0xffffffff;
function bounded(iterable, limit, label) {
  if (!Number.isInteger(limit) || limit < 1 || limit > 65536) throw new LimitError(`${label} limit must be an integer between 1 and 65536`);
  const values = [];
  for (const value of iterable) {
    if (values.length === limit) throw new LimitError(`${label} exceeds its declared limit`, { limit });
    values.push(value);
  }
  return Object.freeze(values);
}
const limits = Object.freeze({ maxStates: 128, maxTransitions: 512, maxValues: 8192, maxCallbacks: 4096 });
function checkedLimits(value = {}) {
  const result = { ...limits, ...value };
  for (const [name, limit] of Object.entries(result)) {
    if (!Object.hasOwn(limits, name)) throw new DefinitionError(`Unknown limit ${name}`);
    if (!Number.isInteger(limit) || limit < 1 || limit > 65536) throw new LimitError(`${name} must be an integer between 1 and 65536`);
  }
  return Object.freeze(result);
}
class Identities {
  constructor() { this.values = new Map(); this.indexes = new Map(); }
  intern(role, value) {
    if (!this.values.has(role)) { this.values.set(role, []); this.indexes.set(role, new Map()); }
    const indexes = this.indexes.get(role);
    if (indexes.has(value)) return indexes.get(value);
    const values = this.values.get(role);
    if (values.length === 65536) throw new LimitError('Identifier inventory exceeds 65536 entries', { role });
    const index = values.length; values.push(value); indexes.set(value, index); return index;
  }
  find(role, value) { const index = this.indexes.get(role); return index !== undefined && index.has(value) ? index.get(value) : UNKNOWN_EVENT; }
  resolve(role, index) {
    const values = this.values.get(role);
    if (values === undefined || index >= values.length) throw new Error('Kernel returned an unknown native identifier');
    return values[index];
  }
}
class BehaviorRef {
  constructor(story, implementation) { this.story = story; this.implementation = implementation; Object.freeze(this); }
}
const BehaviorRole = Object.freeze({ Action: 'action', Condition: 'condition', Invariant: 'invariant',
  Effect: 'effect', Output: 'output', Capability: 'capability', Assertion: 'assertion' });
class BehaviorRegistry {
  constructor({ implementations = [], maxBindings = 128 } = {}) {
    if (!Number.isInteger(maxBindings) || maxBindings < 1 || maxBindings > 4096) throw new LimitError('maxBindings must be between 1 and 4096');
    this.maxBindings = maxBindings;
    this.implementations = bounded(implementations, maxBindings, 'Story registry').map(value => Object.freeze({ ...value }));
    Object.freeze(this.implementations); Object.freeze(this);
  }
}
class Compiler {
  constructor(limits) { this.limits = limits; this.ids = new Identities(); this.callbacks = []; }
  reference(value) { return { story: this.ids.intern('story', value.story), implementation: this.ids.intern('implementation', value.implementation) }; }
  implementation(value, update = false) {
    if (value instanceof BehaviorRef) {
      if (update) throw new DefinitionError('Select update mode on the registered implementation');
      return { callback: null, reference: this.reference(value) };
    }
    if (typeof value !== 'function') throw new DefinitionError('A behavior must be a function or an exact BehaviorRef');
    if (value.constructor.name === 'AsyncFunction') throw new DefinitionError('Machine callbacks must be synchronous');
    if (this.callbacks.length === this.limits.maxCallbacks) throw new LimitError('Callback inventory exceeds maxCallbacks');
    const callback = this.callbacks.length; this.callbacks.push(Object.freeze({ function: value, update }));
    return { callback, reference: null };
  }
  action(value) {
    if ((value.apply === undefined) === (value.update === undefined)) throw new DefinitionError('An action needs exactly one apply or update implementation');
    return { id: this.ids.intern('action', value.id), implementation: value.update === undefined
      ? this.implementation(value.apply) : this.implementation(value.update, true) };
  }
  named(value, role) { return { id: this.ids.intern(role, role === 'event' ? value.kind : value.id), implementation: this.implementation(value.describe) }; }
  condition(value, depth = 1) {
    if (depth > 32) throw new DefinitionError('Condition nesting exceeds 32');
    if (['predicate', 'allOf', 'anyOf', 'not'].filter(key => value[key] !== undefined).length !== 1) throw new DefinitionError('A condition needs exactly one expression');
    let expression;
    if (value.predicate !== undefined) expression = { kind: 'predicate', implementation: this.implementation(value.predicate) };
    else if (value.not !== undefined) expression = { kind: 'not', child: this.condition(value.not, depth + 1) };
    else {
      const all = value.allOf !== undefined;
      expression = { kind: all ? 'all' : 'any', children: bounded(all ? value.allOf : value.anyOf, 16, 'condition operands').map(child => this.condition(child, depth + 1)) };
    }
    return { id: this.ids.intern('condition', value.id), expression };
  }
  state(value) {
    return { id: this.ids.intern('state', value.id), terminal: value.terminal === undefined ? false : value.terminal,
      invariant: value.invariant === undefined ? null : this.implementation(value.invariant),
      entry_actions: bounded(value.entryActions === undefined ? [] : value.entryActions, 16, 'entry actions').map(v => this.action(v)),
      exit_actions: bounded(value.exitActions === undefined ? [] : value.exitActions, 16, 'exit actions').map(v => this.action(v)),
      entry_effects: bounded(value.entryEffects === undefined ? [] : value.entryEffects, 16, 'entry effects').map(v => this.named(v, 'effect')),
      exit_effects: bounded(value.exitEffects === undefined ? [] : value.exitEffects, 16, 'exit effects').map(v => this.named(v, 'effect')) };
  }
  transition(value) {
    return { id: this.ids.intern('transition', value.id), source: this.ids.intern('state', value.source),
      target: this.ids.intern('state', value.target), event: this.ids.intern('event', value.event),
      internal: value.internal === undefined ? false : value.internal,
      guard: value.guard === undefined ? null : this.condition(value.guard),
      postconditions: bounded(value.postconditions === undefined ? [] : value.postconditions, 16, 'postconditions').map(v => this.condition(v)),
      actions: bounded(value.actions === undefined ? [] : value.actions, 16, 'actions').map(v => this.action(v)),
      effects: bounded(value.effects === undefined ? [] : value.effects, 16, 'effects').map(v => this.named(v, 'effect')),
      outputs: bounded(value.outputs === undefined ? [] : value.outputs, 16, 'outputs').map(v => this.named(v, 'event')) };
  }
  disposition(value) {
    return { id: this.ids.intern('disposition', value.id), source: this.ids.intern('state', value.source),
      event: this.ids.intern('event', value.event), condition: this.condition(value.condition), reason: value.reason };
  }
  behaviors(registry) {
    return registry.implementations.map(value => ({ reference: this.reference(value.reference),
      role: value.role, callback: this.implementation(value.callback, value.update).callback }));
  }
}
module.exports = { bounded, checkedLimits, Compiler, BehaviorRef, BehaviorRegistry, BehaviorRole };
