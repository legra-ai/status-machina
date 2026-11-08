'use strict';
const native = require('./native.cjs');
const { Context } = require('./context.cjs');
const { Compiler, BehaviorRegistry, bounded, checkedLimits } = require('./compile.cjs');
const { snapshot } = require('./snapshot.cjs');
const checked = require('./specification.cjs');
const { DefinitionError, DisposedError, translate } = require('./errors.cjs');
const TOKEN = Symbol('owned machine creation');
function revision(value) {
  if (value === undefined) return undefined;
  if (typeof value !== 'bigint' || value < 0n || value > 0xffffffffffffffffn) throw new TypeError('expectedRevision must be a nonnegative uint64 BigInt');
  return value.toString();
}
class Definition {
  #internal;
  constructor(specification) {
    checked.fields(specification, ['id', 'initial', 'states', 'transitions', 'dispositions', 'behaviors', 'snapshot', 'inputSnapshot', 'limits'], 'definition', ['id', 'initial', 'states', 'transitions']);
    if (!Object.hasOwn(specification, 'id') || !Object.hasOwn(specification, 'initial')) throw new DefinitionError('Definition requires explicit id and initial values');
    const limits = checkedLimits(specification.limits);
    const compiler = new Compiler(limits);
    const states = Object.freeze(bounded(specification.states, limits.maxStates, 'state inventory').map(checked.state));
    const transitions = Object.freeze(bounded(specification.transitions, limits.maxTransitions, 'transition inventory').map(checked.transition));
    const dispositions = Object.freeze(bounded(specification.dispositions === undefined ? [] : specification.dispositions, 128, 'disposition inventory').map(checked.disposition));
    const behaviors = specification.behaviors === undefined ? new BehaviorRegistry() : specification.behaviors;
    const copy = specification.snapshot === undefined ? snapshot : specification.snapshot;
    const inputCopy = specification.inputSnapshot === undefined ? snapshot : specification.inputSnapshot;
    if (typeof copy !== 'function' || typeof inputCopy !== 'function') throw new DefinitionError('snapshot and inputSnapshot must be functions');
    for (const callback of [copy, inputCopy]) if (callback.constructor.name === 'AsyncFunction') throw new DefinitionError('Snapshot functions must be synchronous');
    compiler.ids.intern('definition', specification.id);
    const source = { initial: compiler.ids.intern('state', specification.initial),
      max_states: limits.maxStates, max_transitions: limits.maxTransitions, max_bindings: behaviors.maxBindings,
      states: states.map(value => compiler.state(value)), transitions: transitions.map(value => compiler.transition(value)),
      dispositions: dispositions.map(value => compiler.disposition(value)), behaviors: compiler.behaviors(behaviors) };
    let executable;
    try { executable = new native.NativeDefinition(JSON.stringify(source)); }
    catch (error) { translate(error, compiler.ids); }
    this.#internal = Object.freeze({ compiler, callbacks: Object.freeze(compiler.callbacks), snapshot: copy,
      inputSnapshot: inputCopy, limits, executable });
    Object.defineProperties(this, { id: { value: specification.id, enumerable: true },
      initial: { value: specification.initial, enumerable: true }, states: { value: states, enumerable: true },
      transitions: { value: transitions, enumerable: true } });
    Object.freeze(this);
  }
  _internal() { return this.#internal; }
  instantiate(options) {
    if (!Object.hasOwn(options, 'id') || !Object.hasOwn(options, 'data')) throw new DefinitionError('instantiate requires explicit id and data values');
    const context = new Context(this, options.id);
    try {
      context.native = this.#internal.executable.instantiate(context.allocate(options.data), this.#internal.limits.maxValues, context.invoke);
      if (context.arenaId !== context.native.arenaId()) throw new Error('Native arena identity changed during admission');
    } catch (error) { context.values.clear(); translate(error, this.#internal.compiler.ids); }
    context.sweep();
    return new Machine(TOKEN, context);
  }
}
class Machine {
  #context;
  constructor(token, context) {
    if (token !== TOKEN) throw new DefinitionError('Create machines through Definition.instantiate');
    this.#context = context;
  }
  get id() {
    if (this.#context.closed) throw new DisposedError('The machine has been disposed');
    return this.#context.identity;
  }
  get state() { return this.#context.operation(context => context.ids.resolve('state', context.native.state())); }
  get revision() { return this.#context.operation(context => BigInt(context.native.revision())); }
  get data() { return this.#context.operation(context => context.resolve(JSON.parse(context.native.snapshot(context.invoke)))); }
  #outcome(encoded, eventKind) {
    const result = JSON.parse(encoded);
    const context = this.#context;
    const ids = context.ids;
    if (result.kind === 'refused') return Object.freeze({ kind: 'refused', state: ids.resolve('state', result.state), event: eventKind });
    if (result.kind === 'ignored') return Object.freeze({ kind: 'ignored', disposition: ids.resolve('disposition', result.disposition),
      state: ids.resolve('state', result.state), event: eventKind, reason: result.reason });
    if (result.kind !== 'transitioned') throw new Error('Unrecognized native dispatch outcome');
    const raw = result.record;
    const record = Object.freeze({ transition: ids.resolve('transition', raw.transition), source: ids.resolve('state', raw.from_state),
      target: ids.resolve('state', raw.to_state), previousRevision: BigInt(raw.previous_revision), revision: BigInt(raw.revision) });
    const effects = Object.freeze(result.effects.map(value => Object.freeze({ id: ids.resolve('effect', value.id), payload: context.resolve(value.payload) })));
    const outputs = Object.freeze(result.outputs.map(value => Object.freeze({ kind: ids.resolve('event', value.id), payload: context.resolve(value.payload) })));
    return Object.freeze({ kind: 'transitioned', record, effects, outputs });
  }
  dispatch(event, facts = undefined, options = {}) {
    return this.#context.operation(context => this.#outcome(context.native.dispatch(context.ids.find('event', event.kind),
      context.allocate(event.payload), context.allocate(facts), context.invoke, revision(options.expectedRevision)), event.kind));
  }
  evaluate(event, facts = undefined, options = {}) {
    return this.#context.operation(context => new Decision(TOKEN, context,
      context.native.evaluate(context.ids.find('event', event.kind), context.allocate(event.payload), context.allocate(facts),
        context.invoke, revision(options.expectedRevision)), event.kind));
  }
  accept(decision) {
    return this.#context.operation(context => {
      const owned = decision._take(TOKEN);
      try { return this.#outcome(context.native.accept(owned.native), owned.eventKind); }
      finally { if (owned.context !== context) owned.context.sweep(); }
    });
  }
  dispose() {
    this.#context.operation(context => {
      if (!context.closed) { context.native.dispose(); context.detach(); }
    }, true);
  }
  [Symbol.dispose]() { this.dispose(); }
}
class Decision {
  #owned;
  constructor(token, context, executable, eventKind) {
    if (token !== TOKEN) throw new DefinitionError('Create decisions through Machine.evaluate');
    this.#owned = { context, native: executable, eventKind };
  }
  get expectedRevision() {
    if (this.#owned === undefined) throw new DisposedError('The decision has already been consumed');
    return BigInt(this.#owned.native.expectedRevision());
  }
  _take(token) {
    if (token !== TOKEN) throw new DefinitionError('Decision access is private');
    if (this.#owned === undefined) throw new DisposedError('The decision has already been consumed');
    const value = this.#owned; this.#owned = undefined; return value;
  }
  dispose() {
    if (this.#owned === undefined) return;
    const owned = this.#owned;
    owned.context.operation(() => { owned.native.dispose(); this.#owned = undefined; }, true);
  }
  [Symbol.dispose]() { this.dispose(); }
}
module.exports = { Definition, Machine, Decision };
