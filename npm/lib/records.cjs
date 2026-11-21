'use strict';
// Portable, data-only definition records. A record holds native identities and
// exact BehaviorRef selections only: no functions, module specifiers or string
// codec. DefinitionRecord.bind resolves every use site through the caller's
// BehaviorRegistry with the Rust kernel's DefinitionRecord::bind.
const { BehaviorRef, BehaviorRegistry, ForbiddenEdge, bounded } = require('./compile.cjs');
const { bindRecord } = require('./machine.cjs');
const { ConditionRecordError, DefinitionError, LimitError, RecordError } = require('./errors.cjs');
const MAX_CONDITION_DEPTH = 32;
const PHASE_LIMIT = 16;
// Lowering to the checked specification is private to this module.
const LOWER = Symbol('lower record');
function fields(value, allowed, label, required = []) {
  if (value === null || typeof value !== 'object') throw new RecordError(`${label} must be an options object`);
  for (const key of Reflect.ownKeys(value)) if (!allowed.includes(key)) throw new RecordError(`Unknown ${label} field ${String(key)}`);
  for (const key of required) if (!Object.hasOwn(value, key)) throw new RecordError(`${label} requires ${key}`);
}
function reference(value, label) {
  if (!(value instanceof BehaviorRef)) throw new RecordError(`${label} must be an exact BehaviorRef; records never carry functions`);
  return value;
}
function uses(values, label) {
  const result = bounded(values === undefined ? [] : values, PHASE_LIMIT, label);
  for (const value of result) if (!(value instanceof BehaviorUse)) throw new RecordError(`${label} must contain BehaviorUse values`);
  return result;
}
function condition(value, label) {
  if (!(value instanceof PredicateRecord || value instanceof AllRecord || value instanceof AnyRecord || value instanceof NotRecord)) {
    throw new RecordError(`${label} must be a condition record`);
  }
  return value;
}
function conditions(values, label) {
  const result = bounded(values === undefined ? [] : values, PHASE_LIMIT, label);
  for (const value of result) condition(value, label);
  return result;
}
function flag(value, label) {
  if (value === undefined) return false;
  if (typeof value !== 'boolean') throw new RecordError(`${label} must be a boolean`);
  return value;
}
// Bound native recursion with the kernel's own depth rule and identity.
function lower(value, depth = 1) {
  if (depth > MAX_CONDITION_DEPTH) throw new ConditionRecordError('condition nesting exceeds 32 levels', { condition: value.id });
  return value[LOWER](depth);
}
function action(use) { return { id: use.id, apply: use.reference }; }
function effect(use) { return { id: use.id, describe: use.reference }; }
class BehaviorUse {
  constructor(id, behavior) { this.id = id; this.reference = reference(behavior, 'BehaviorUse reference'); Object.freeze(this); }
}
class PredicateRecord {
  constructor(use) {
    if (!(use instanceof BehaviorUse)) throw new RecordError('PredicateRecord requires a BehaviorUse');
    this.kind = 'predicate'; this.use = use; Object.freeze(this);
  }
  get id() { return this.use.id; }
  [LOWER]() { return { id: this.use.id, predicate: this.use.reference }; }
}
class AllRecord {
  constructor(id, operands) { this.kind = 'all'; this.id = id; this.operands = conditions(operands, 'all-record operands'); Object.freeze(this); }
  [LOWER](depth) { return { id: this.id, allOf: this.operands.map(item => lower(item, depth + 1)) }; }
}
class AnyRecord {
  constructor(id, operands) { this.kind = 'any'; this.id = id; this.operands = conditions(operands, 'any-record operands'); Object.freeze(this); }
  [LOWER](depth) { return { id: this.id, anyOf: this.operands.map(item => lower(item, depth + 1)) }; }
}
class NotRecord {
  constructor(id, operand) { this.kind = 'not'; this.id = id; this.operand = condition(operand, 'NotRecord operand'); Object.freeze(this); }
  [LOWER](depth) { return { id: this.id, not: lower(this.operand, depth + 1) }; }
}
class StateRecord {
  constructor(options) {
    fields(options, ['id', 'terminal', 'invariant', 'entryActions', 'exitActions', 'entryEffects', 'exitEffects'], 'state record', ['id']);
    this.id = options.id; this.terminal = flag(options.terminal, 'StateRecord terminal');
    this.invariant = options.invariant === undefined ? undefined : reference(options.invariant, 'StateRecord invariant');
    this.entryActions = uses(options.entryActions, 'entry actions'); this.exitActions = uses(options.exitActions, 'exit actions');
    this.entryEffects = uses(options.entryEffects, 'entry effects'); this.exitEffects = uses(options.exitEffects, 'exit effects');
    Object.freeze(this);
  }
  [LOWER]() {
    const result = { id: this.id, terminal: this.terminal, entryActions: this.entryActions.map(action),
      exitActions: this.exitActions.map(action), entryEffects: this.entryEffects.map(effect), exitEffects: this.exitEffects.map(effect) };
    if (this.invariant !== undefined) result.invariant = this.invariant;
    return result;
  }
}
class TransitionDefinitionRecord {
  constructor(options) {
    fields(options, ['id', 'source', 'event', 'target', 'internal', 'guard', 'postconditions', 'actions', 'effects', 'outputs'],
      'transition record', ['id', 'source', 'event', 'target']);
    this.id = options.id; this.source = options.source; this.event = options.event; this.target = options.target;
    this.internal = flag(options.internal, 'TransitionDefinitionRecord internal');
    this.guard = options.guard === undefined ? undefined : condition(options.guard, 'TransitionDefinitionRecord guard');
    this.postconditions = conditions(options.postconditions, 'postconditions');
    this.actions = uses(options.actions, 'actions'); this.effects = uses(options.effects, 'effects'); this.outputs = uses(options.outputs, 'outputs');
    Object.freeze(this);
  }
  [LOWER]() {
    const result = { id: this.id, source: this.source, event: this.event, target: this.target, internal: this.internal,
      postconditions: this.postconditions.map(item => lower(item)), actions: this.actions.map(action),
      effects: this.effects.map(effect), outputs: this.outputs.map(use => ({ kind: use.id, describe: use.reference })) };
    if (this.guard !== undefined) result.guard = lower(this.guard);
    return result;
  }
}
class DispositionRecord {
  constructor(options) {
    fields(options, ['id', 'source', 'event', 'condition', 'reason'], 'disposition record', ['id', 'source', 'event', 'condition', 'reason']);
    if (typeof options.reason !== 'string') throw new RecordError('DispositionRecord reason must be a string');
    this.id = options.id; this.source = options.source; this.event = options.event;
    this.condition = condition(options.condition, 'DispositionRecord condition'); this.reason = options.reason;
    Object.freeze(this);
  }
  [LOWER]() { return { id: this.id, source: this.source, event: this.event, condition: lower(this.condition), reason: this.reason }; }
}
function limit(value, fallback, name) {
  if (value === undefined) return fallback;
  if (!Number.isInteger(value) || value < 1 || value > 65536) throw new LimitError(`${name} must be an integer between 1 and 65536`);
  return value;
}
function inventory(values, max, label, Type) {
  const result = bounded(values, max, `${label} record inventory`);
  for (const value of result) if (!(value instanceof Type)) throw new RecordError(`DefinitionRecord ${label} must contain ${Type.name} values`);
  return result;
}
class DefinitionRecord {
  constructor(options) {
    fields(options, ['id', 'initial', 'states', 'transitions', 'dispositions', 'forbiddenEdges', 'maxStates', 'maxTransitions'],
      'definition record', ['id', 'initial', 'states', 'transitions']);
    this.id = options.id; this.initial = options.initial;
    this.maxStates = limit(options.maxStates, 128, 'maxStates'); this.maxTransitions = limit(options.maxTransitions, 512, 'maxTransitions');
    this.states = inventory(options.states, this.maxStates, 'states', StateRecord);
    this.transitions = inventory(options.transitions, this.maxTransitions, 'transitions', TransitionDefinitionRecord);
    this.dispositions = inventory(options.dispositions === undefined ? [] : options.dispositions, 128, 'dispositions', DispositionRecord);
    this.forbiddenEdges = inventory(options.forbiddenEdges === undefined ? [] : options.forbiddenEdges, 128, 'forbiddenEdges', ForbiddenEdge);
    Object.freeze(this);
  }
  bind(behaviors, options = {}) {
    if (!(behaviors instanceof BehaviorRegistry)) throw new DefinitionError('Records bind only against an explicit BehaviorRegistry');
    fields(options, ['snapshot', 'inputSnapshot', 'maxValues'], 'record binding');
    const specification = { id: this.id, initial: this.initial, behaviors,
      states: this.states.map(value => value[LOWER]()), transitions: this.transitions.map(value => value[LOWER]()),
      dispositions: this.dispositions.map(value => value[LOWER]()), forbiddenEdges: this.forbiddenEdges,
      limits: { maxStates: this.maxStates, maxTransitions: this.maxTransitions, maxCallbacks: 4096,
        ...(options.maxValues === undefined ? {} : { maxValues: options.maxValues }) } };
    if (options.snapshot !== undefined) specification.snapshot = options.snapshot;
    if (options.inputSnapshot !== undefined) specification.inputSnapshot = options.inputSnapshot;
    return bindRecord(specification, this);
  }
}
module.exports = { BehaviorUse, PredicateRecord, AllRecord, AnyRecord, NotRecord, StateRecord, TransitionDefinitionRecord,
  DispositionRecord, DefinitionRecord };
