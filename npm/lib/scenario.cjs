'use strict';
// Bounded replay of finite caller-supplied scenarios, decided by the kernel's
// ScenarioLimits, ScenarioSet, ScenarioAssertion and ScenarioReplay. This module
// interns native identities (Map semantics) and resolves the report back to them.
const native = require('./native.cjs');
const { Context } = require('./context.cjs');
const { BehaviorRef } = require('./compile.cjs');
const { Definition } = require('./machine.cjs');
const errors = require('./errors.cjs');
// Kernel ceilings: they bound how many native entries are read, never what is accepted.
const HARD_MAX_SCENARIOS = 1024;
const HARD_MAX_STEPS_PER_SCENARIO = 4096;
const HARD_MAX_ASSERTIONS = 128;
const LIMITS = Object.freeze([['maxScenarios', 'max_scenarios'], ['maxStepsPerScenario', 'max_steps_per_scenario'],
  ['maxNodes', 'max_nodes'], ['maxOutputs', 'max_outputs'], ['maxTraceEntries', 'max_trace_entries']]);
const FAILURES = Object.freeze({
  'limits/outside_supported_range': errors.ScenarioLimitsError,
  'set/empty': errors.EmptyScenarioSetError,
  'set/scenario_limit_exceeded': errors.ScenarioCountLimitError,
  'set/step_limit_exceeded': errors.ScenarioStepLimitError,
  'set/total_node_limit_exceeded': errors.ScenarioNodeLimitError,
  'assertion/missing_predicate': errors.MissingAssertionPredicateError,
  'assertion/unknown_behavior': errors.UnknownBehaviorError,
  'assertion/unknown_implementation': errors.UnknownImplementationError,
  'assertion/role_mismatch': errors.RoleMismatchError,
  'replay/assertion_limit_exceeded': errors.AssertionLimitError,
});
const ScenarioFailure = Object.freeze({ EvaluationFailed: 'evaluation_failed', InvalidInitialData: 'invalid_initial_data',
  Ambiguous: 'ambiguous', InvariantFailed: 'invariant_failed', PostconditionFailed: 'postcondition_failed',
  ActionFailed: 'action_failed', RevisionExhausted: 'revision_exhausted', DefinitionMismatch: 'definition_mismatch',
  InstanceMismatch: 'instance_mismatch', StaleRevision: 'stale_revision' });
const ScenarioLimit = Object.freeze({ Outputs: 'outputs', TraceEntries: 'trace_entries' });

function scenarioFailure(error) {
  if (!errors.isNative(error)) return undefined;
  const info = JSON.parse(error.message);
  return info.code === 'scenario' ? info : undefined;
}
function raise(info, context = {}) {
  const Type = FAILURES[`${info.family}/${info.reason}`];
  if (Type === undefined) throw new Error('Unrecognized native scenario failure');
  const full = { ...context, reason: info.reason };
  if (info.limit !== null) full.limit = info.limit;
  if (info.entry !== null) full.entry = info.entry;
  if (info.expected_role !== null) full.expected = info.expected_role;
  if (info.actual_role !== null) full.actual = info.actual_role;
  throw new Type(info.message, full);
}
function call(body, context) {
  try { return body(); } catch (error) {
    const info = scenarioFailure(error);
    if (info === undefined) throw error;
    raise(info, typeof context === 'function' ? context(info) : context);
  }
}
function options(value, allowed, label, required = []) {
  if (value === null || typeof value !== 'object') throw new TypeError(`${label} must be an options object`);
  for (const key of Reflect.ownKeys(value)) if (!allowed.includes(key)) throw new TypeError(`Unknown ${label} field ${String(key)}`);
  for (const key of required) if (!Object.hasOwn(value, key)) throw new TypeError(`${label} requires ${key}`);
}
/** At most `limit` + 1 entries, so the kernel decides the bound and endless iterables stop. */
function read(values, limit) {
  if (values === null || typeof values !== 'object' || typeof values[Symbol.iterator] !== 'function') throw new TypeError('Expected an iterable');
  const result = [];
  for (const value of values) {
    result.push(value);
    if (result.length > limit) break;
  }
  return result;
}
/** A bound the kernel can receive; anything wider is outside every ceiling too. */
function bound(value) { return value < 0 ? 0 : Math.min(value, Number.MAX_SAFE_INTEGER); }
const DEFAULTS = Object.freeze(JSON.parse(native.scenarioLimits('{}')));

/** Positive replay budgets, validated by the kernel; defaults are the kernel's own. */
class ScenarioLimits {
  constructor(value = {}) {
    options(value, LIMITS.map(([name]) => name), 'scenario limits');
    const source = {};
    for (const [name, wire] of LIMITS) {
      const limit = value[name] === undefined ? DEFAULTS[wire] : value[name];
      if (!Number.isInteger(limit)) throw new TypeError(`${name} must be an integer`);
      this[name] = limit; source[wire] = bound(limit);
    }
    call(() => native.scenarioLimits(JSON.stringify(source)), Object.fromEntries(LIMITS.map(([name]) => [name, this[name]])));
    Object.freeze(this);
  }
  _encode() { return Object.fromEntries(LIMITS.map(([name, wire]) => [wire, bound(this[name])])); }
}
function scenarioStep(value) {
  options(value, ['event', 'facts'], 'scenario step', ['event']);
  const event = value.event;
  if (event === null || typeof event !== 'object' || !Object.hasOwn(event, 'kind')) throw new TypeError('A scenario step needs an event with a kind');
  return Object.freeze({ event: Object.freeze({ kind: event.kind, payload: event.payload }), facts: value.facts });
}
function scenario(value) {
  options(value, ['id', 'steps'], 'scenario', ['id', 'steps']);
  return Object.freeze({ id: value.id, steps: Object.freeze(read(value.steps, HARD_MAX_STEPS_PER_SCENARIO).map(scenarioStep)) });
}
/** A non-empty bounded set of scenarios, checked by the kernel's ScenarioSet builder. */
class ScenarioSet {
  constructor(value) {
    options(value, ['scenarios', 'limits'], 'scenario set', ['scenarios']);
    const limits = value.limits === undefined ? new ScenarioLimits() : value.limits;
    if (!(limits instanceof ScenarioLimits)) throw new TypeError('limits must be ScenarioLimits');
    const scenarios = Object.freeze(read(value.scenarios, Math.min(limits.maxScenarios, HARD_MAX_SCENARIOS)).map(scenario));
    call(() => native.scenarioSet(JSON.stringify({ limits: limits._encode(), steps: scenarios.map(item => item.steps.length) })),
      info => info.entry === null ? {} : { scenario: scenarios[info.entry].id });
    this.scenarios = scenarios; this.limits = limits;
    Object.freeze(this);
  }
}
/** One independently named safety predicate, true while the property holds. */
class ScenarioAssertion {
  constructor(value) {
    options(value, ['id', 'evaluate'], 'scenario assertion', ['id']);
    const evaluate = value.evaluate;
    if (evaluate !== undefined && !(evaluate instanceof BehaviorRef)) {
      if (typeof evaluate !== 'function') throw new TypeError('evaluate must be a function or an exact BehaviorRef');
      if (evaluate.constructor.name === 'AsyncFunction') throw new TypeError('Scenario assertions must be synchronous');
    }
    call(() => native.scenarioAssertion(evaluate !== undefined), { assertion: value.id });
    this.id = value.id; this.evaluate = evaluate;
    Object.freeze(this);
  }
}
/** One replay arena: every input is leased up front; candidates are swept when it fills. */
class Replay extends Context {
  constructor(definition, identity, scenarios, assertions) {
    super(definition, identity);
    this.scenarios = scenarios; this.assertions = assertions; this.running = false;
  }
  allocate(value) {
    // Only leased keys survive a sweep, and every earlier key is leased once replay runs.
    if (this.running && this.values.size === this.capacity) this.sweep();
    return super.allocate(value);
  }
  lower(evaluate, position) {
    if (evaluate === undefined) return null;
    // An unknown Story or implementation reaches the kernel as an absent index.
    if (evaluate instanceof BehaviorRef) return { callback: null, reference: { story: this.ids.find('story', evaluate.story),
      implementation: this.ids.find('implementation', evaluate.implementation) } };
    return { callback: this.definition._internal().callbacks.length + position, reference: null };
  }
  observation(raw) {
    const ids = this.ids;
    const owner = this.scenarios[raw.scenario];
    let outcome;
    if (raw.kind === 'transitioned') outcome = { kind: 'transitioned', transition: ids.resolve('transition', raw.transition) };
    else if (raw.kind === 'ignored') outcome = { kind: 'ignored', disposition: ids.resolve('disposition', raw.disposition) };
    else outcome = { kind: raw.kind };
    return Object.freeze({ scenario: owner.id, step: raw.step, inputEvent: owner.steps[raw.step].event.kind,
      before: ids.resolve('state', raw.before), after: ids.resolve('state', raw.after), outcome: Object.freeze(outcome),
      outputEvents: Object.freeze(raw.outputs.map(value => ids.resolve('event', value))),
      effects: Object.freeze(raw.effects.map(value => ids.resolve('effect', value))) });
  }
  callback(encoded) {
    const request = JSON.parse(encoded);
    if (request.kind !== 'assertion') return super.callback(encoded);
    const registered = this.definition._internal().callbacks;
    const evaluate = request.callback < registered.length ? registered[request.callback].function
      : this.assertions[request.callback - registered.length].evaluate;
    const result = evaluate(this.observation(request.observation));
    if (result !== null && result !== undefined && typeof result.then === 'function') throw new TypeError('Scenario assertions must return synchronously, not a Promise');
    if (typeof result !== 'boolean') throw new TypeError('Scenario assertions must return boolean');
    return JSON.stringify({ kind: 'bool', value: result });
  }
  result(raw) {
    const steps = values => Object.freeze(values.map(value => this.observation(value)));
    if (raw.kind === 'complete_within_scenarios') return Object.freeze({ kind: raw.kind, scenarios: raw.scenarios, nodes: raw.nodes });
    const owner = this.scenarios[raw.scenario].id;
    if (raw.kind === 'counterexample') return Object.freeze({ kind: raw.kind, trace: Object.freeze({ scenario: owner,
      assertion: this.assertions[raw.assertion].id, steps: steps(raw.steps) }) });
    if (raw.kind === 'limit_reached') return Object.freeze({ kind: raw.kind, scenario: owner, step: raw.step, limit: raw.limit });
    if (raw.kind === 'execution_failed') return Object.freeze({ kind: raw.kind, scenario: owner, step: raw.step,
      failure: raw.failure, steps: steps(raw.steps) });
    throw new Error('Unrecognized native scenario result');
  }
}
/** Caller-driven replay of explicit finite scenarios on fresh ephemeral instances. */
const ScenarioReplay = Object.freeze({
  run(definition, value) {
    if (!(definition instanceof Definition)) throw new TypeError('ScenarioReplay.run needs a Definition');
    options(value, ['id', 'data', 'scenarios', 'assertions'], 'scenario replay', ['id', 'data', 'scenarios']);
    if (!(value.scenarios instanceof ScenarioSet)) throw new TypeError('scenarios must be a ScenarioSet');
    const assertions = read(value.assertions === undefined ? [] : value.assertions, HARD_MAX_ASSERTIONS);
    for (const assertion of assertions) if (!(assertion instanceof ScenarioAssertion)) throw new TypeError('assertions must be ScenarioAssertion values');
    const context = new Replay(definition, value.id, value.scenarios.scenarios, assertions);
    const executable = definition._internal().executable;
    const arena = new native.NativeArena(definition._internal().limits.maxValues);
    context.native = arena; context.arenaId = arena.arenaId();
    const ids = context.ids;
    // Repeated input values share one lease; callbacks only ever see snapshots of them.
    const keys = new Map();
    const lease = input => { if (!keys.has(input)) keys.set(input, context.allocate(input)); return keys.get(input); };
    try {
      const source = { limits: value.scenarios.limits._encode(), initial: context.allocate(value.data),
        scenarios: value.scenarios.scenarios.map(item => ({ steps: item.steps.map(step => ({ event: ids.find('event', step.event.kind),
          payload: lease(step.event.payload), facts: lease(step.facts) })) })),
        assertions: assertions.map((assertion, position) => ({ id: position, implementation: context.lower(assertion.evaluate, position) })) };
      context.running = true;
      let raw;
      try { raw = JSON.parse(executable.replay(arena, JSON.stringify(source), context.invoke)); }
      catch (error) {
        const info = scenarioFailure(error);
        if (info === undefined) errors.translate(error, ids);
        const extra = {};
        if (info.entry !== null) {
          const assertion = assertions[info.entry];
          extra.assertion = assertion.id;
          if (assertion.evaluate instanceof BehaviorRef) { extra.story = assertion.evaluate.story; extra.implementation = assertion.evaluate.implementation; }
        }
        raise(info, extra);
      }
      return Object.freeze({ result: context.result(raw) });
    } finally { context.running = false; context.values.clear(); context.detach(); }
  },
});
module.exports = { ScenarioLimits, ScenarioSet, ScenarioAssertion, ScenarioReplay, ScenarioFailure, ScenarioLimit };
