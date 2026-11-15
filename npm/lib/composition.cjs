'use strict';
// Explicit, bounded composition of native machines through the Rust kernel.
// The kernel's InstanceRegistry, Composition, CapabilitySnapshot and
// BoundComposition decide every rule; this module lowers native values to
// private indexes and resolves the kernel's results back to them.
const { markAsUncloneable } = require('node:worker_threads');
const native = require('./native.cjs');
const errors = require('./errors.cjs');
const { decodeOutcome, contexts, reports, Machine } = require('./machine.cjs');
const { InstanceAddress, Route, InstanceRegistry, Delivery, PropagationLimits, HARD_MAX_INSTANCES, HARD_MAX_ROUTES,
  read, same, representable } = require('./routing.cjs');
const UNKNOWN = 0xffffffff;
const HARD_MAX_BATCH = 256;
const HARD_MAX_OBSERVATIONS = 1024;
const TOKEN = Symbol('composition internals');
const KINDS = new Map([
  ...['invalid_limit', 'limit_below_current_size', 'limit_exceeded', 'instance_event_limit_exceeded',
    'aggregate_event_limit_exceeded'].map(reason => [`registry:${reason}`, errors.InstanceRegistryLimitError]),
  ['registry:address_mismatch', errors.AddressMismatchError], ['registry:duplicate_instance', errors.DuplicateInstanceError],
  ...['invalid_limits', 'limit_below_current_size', 'route_limit_exceeded'].map(reason => [`definition:${reason}`, errors.CompositionLimitError]),
  ['definition:duplicate_route', errors.DuplicateRouteError],
  ['definition:unregistered_source', errors.UnregisteredRouteAddressError],
  ['definition:unregistered_destination', errors.UnregisteredRouteAddressError],
  ['definition:undeclared_source_event', errors.UndeclaredRouteEventError],
  ['definition:undeclared_destination_event', errors.UndeclaredRouteEventError],
  ['batch:unregistered_source', errors.UnregisteredSourceError], ['batch:batch_limit_exceeded', errors.BatchLimitError],
  ['binding:duplicate_instance', errors.DuplicateBindingError], ['binding:unregistered_instance', errors.UnregisteredBindingError],
  ['binding:identity_or_inventory_mismatch', errors.BindingMismatchError], ['binding:signature_mismatch', errors.BindingMismatchError],
  ['binding:missing_instance', errors.MissingBindingError], ['binding:aliased_instance', errors.AliasedBindingError],
  ['limits:invalid_limits', errors.PropagationLimitsError],
  ...['invalid_limit', 'limit_below_current_size', 'limit_exceeded'].map(reason => [`snapshot:${reason}`, errors.CapabilitySnapshotLimitError]),
  ['snapshot:instance_mismatch', errors.ReportInstanceMismatchError], ['snapshot:duplicate_revision', errors.DuplicateObservationError],
  ['snapshot:duplicate_capability', errors.DuplicateObservationError], ['snapshot:foreign_snapshot', errors.ForeignSnapshotError],
]);
const FAMILIES = Object.freeze({ registry: errors.InstanceRegistryError, definition: errors.CompositionDefinitionError,
  route: errors.CompositionDefinitionError, batch: errors.RouteBatchError, binding: errors.CompositionBindingError,
  limits: errors.PropagationLimitsError, snapshot: errors.CapabilitySnapshotError });
/** The composition's native addresses; index `i` is the kernel's address `i`. */
class Table {
  constructor() { this.scopes = new Map(); this.addresses = []; this.indexes = new Map(); this.ids = []; this.wire = []; }
  find(address) { return this.indexes.get(address.scope)?.get(address.instance); }
  scope(value) { if (!this.scopes.has(value)) this.scopes.set(value, this.scopes.size); return this.scopes.get(value); }
  add(address, machine = null, ids = undefined) {
    const known = this.find(address);
    if (known !== undefined) return known;
    if (!this.indexes.has(address.scope)) this.indexes.set(address.scope, new Map());
    const index = this.addresses.length;
    this.indexes.get(address.scope).set(address.instance, index);
    this.addresses.push(address); this.ids.push(ids); this.wire.push({ scope: this.scope(address.scope), machine });
    return index;
  }
}
/** Call-local addresses outside the table, each with its own unknown scope. */
class Call {
  constructor(table) { this.table = table; this.extra = []; this.addresses = []; this.indexes = new Table(); }
  address(value, report = null) {
    if (!(value instanceof InstanceAddress)) throw new TypeError('Composition addresses must be InstanceAddress values');
    const known = this.table.find(value);
    if (known !== undefined) return known;
    const local = this.indexes.find(value);
    if (local !== undefined) return this.table.addresses.length + local;
    this.indexes.add(value);
    this.extra.push({ scope: UNKNOWN - this.extra.length, report }); this.addresses.push(value);
    return this.table.addresses.length + this.extra.length - 1;
  }
  resolve(index) { const table = this.table.addresses; return index < table.length ? table[index] : this.addresses[index - table.length]; }
  ids(index) { return index < this.table.ids.length ? this.table.ids[index] : undefined; }
}
function find(ids, role, value) { return ids === undefined ? UNKNOWN : ids.find(role, value); }
function required(ids) { if (ids === undefined) throw new Error('The kernel returned an unregistered address'); return ids; }
function context(machine) {
  const value = contexts.get(machine);
  if (value === undefined || value.closed || value.native === undefined) throw new errors.DisposedError('The machine has been disposed');
  return value;
}
/** The typed exception for a kernel composition rejection, with native context. */
function failure(error, call, extra = {}) {
  if (!errors.isNative(error)) return error;
  const info = JSON.parse(error.message);
  if (info.code !== 'composition') return errors.exception(error);
  const result = { reason: info.reason, ...extra };
  for (const name of ['registry', 'list', 'entry', 'limit']) if (info[name] !== null) result[name] = info[name];
  if (info.address !== null) result.address = call.resolve(info.address);
  if (info.route !== null && extra.routes !== undefined) { result.route = extra.routes[info.route]; result.index = info.route; }
  if (extra.registries !== undefined && info.registry !== null && info.entry !== null) result.machine = extra.registries[info.registry].entries[info.entry][1];
  delete result.routes; delete result.registries;
  const Type = KINDS.get(`${info.family}:${info.reason}`) ?? FAMILIES[info.family];
  return new Type(info.message, result);
}
/** Caller-observed revisions and capability reports, lowered by one composition. */
class CapabilitySnapshot {
  #native;
  constructor(token, composition, executable, revisions, observed, maxObservations) {
    if (token !== TOKEN) throw new TypeError('Create snapshots through Composition.capabilities');
    this.#native = executable;
    Object.defineProperties(this, { composition: { value: composition }, revisions: { value: revisions, enumerable: true },
      reports: { value: observed, enumerable: true }, maxObservations: { value: maxObservations, enumerable: true } });
    markAsUncloneable(this);
    Object.freeze(this);
  }
  _native(token) { if (token !== TOKEN) throw new TypeError('Snapshot access is private'); return this.#native; }
}
/** Immutable, bounded routing between registered machine instances. */
class Composition {
  #native; #table;
  constructor(value) {
    if (value === null || typeof value !== 'object') throw new TypeError('Composition needs an options object');
    for (const key of Reflect.ownKeys(value)) if (!['sources', 'destinations', 'routes', 'maxRoutes', 'maxBatch'].includes(key)) throw new TypeError(`Unknown composition field ${String(key)}`);
    const { sources, destinations } = value;
    if (!(sources instanceof InstanceRegistry) || !(destinations instanceof InstanceRegistry)) throw new TypeError('sources and destinations must be InstanceRegistry values');
    const maxRoutes = value.maxRoutes === undefined ? 128 : value.maxRoutes;
    const maxBatch = value.maxBatch === undefined ? 128 : value.maxBatch;
    if (!Number.isInteger(maxRoutes) || !Number.isInteger(maxBatch)) throw new TypeError('maxRoutes and maxBatch must be integers');
    const routes = read(value.routes === undefined ? [] : value.routes, maxRoutes, HARD_MAX_ROUTES);
    if (!routes.every(route => route instanceof Route)) throw new TypeError('routes must be Route values');
    const table = new Table();
    const machines = [];
    const registries = { sources, destinations };
    for (const registry of [sources, destinations]) for (const [, machine] of registry.entries) if (!machines.includes(machine)) machines.push(machine);
    for (const registry of [sources, destinations]) {
      for (const [address, machine] of registry.entries) {
        if (same(address.instance, machine.id)) table.add(address, machines.indexOf(machine), context(machine).ids);
        else table.add(address);
      }
    }
    const lowered = routes.map(route => {
      const source = table.add(route.source); const destination = table.add(route.destination);
      const target = table.ids[destination];
      return { source, event: find(table.ids[source], 'event', route.event), destination,
        input_event: find(target, 'event', route.inputEvent),
        capability: route.capability === undefined ? null : find(target, 'capability', route.capability) };
    });
    const registry = registry => ({ max_instances: representable(registry.maxInstances) ?? 0,
      entries: registry.entries.map(([address, machine]) => ({ address: table.find(address), machine: machines.indexOf(machine) })) });
    const spec = { addresses: table.wire, sources: registry(sources), destinations: registry(destinations), routes: lowered,
      max_routes: representable(maxRoutes) ?? 0, max_batch: representable(maxBatch) ?? 0 };
    try { this.#native = new native.NativeComposition(JSON.stringify(spec), machines.map(machine => context(machine).native)); }
    catch (error) { throw failure(error, new Call(table), { routes, registries }); }
    lowered.forEach((wire, index) => {
      if (wire.capability === UNKNOWN) throw new errors.UndeclaredCapabilityError('The route\'s capability is not declared by the destination definition',
        { reason: 'undeclared_capability', route: routes[index], index, capability: routes[index].capability });
    });
    this.#table = table;
    Object.defineProperties(this, { routes: { value: routes, enumerable: true }, maxRoutes: { value: maxRoutes, enumerable: true },
      maxBatch: { value: maxBatch, enumerable: true } });
    markAsUncloneable(this);
    Object.freeze(this);
  }
  _internal(token) { if (token !== TOKEN) throw new TypeError('Composition access is private'); return { native: this.#native, table: this.#table }; }
  #snapshot(capabilities) {
    if (capabilities === undefined) return undefined;
    if (!(capabilities instanceof CapabilitySnapshot)) throw new TypeError('capabilities must be a CapabilitySnapshot');
    return capabilities._native(TOKEN);
  }
  _snapshot(token, capabilities) { if (token !== TOKEN) throw new TypeError('Composition access is private'); return this.#snapshot(capabilities); }
  /** Observe current revisions, then reports from Machine.evaluateCapability. */
  capabilities(value = {}) {
    for (const key of Reflect.ownKeys(value)) if (!['revisions', 'reports', 'maxObservations'].includes(key)) throw new TypeError(`Unknown snapshot field ${String(key)}`);
    const maxObservations = value.maxObservations === undefined ? 256 : value.maxObservations;
    if (!Number.isInteger(maxObservations)) throw new TypeError('maxObservations must be an integer');
    const revisions = read(value.revisions === undefined ? [] : value.revisions, maxObservations, HARD_MAX_OBSERVATIONS);
    const observed = read(value.reports === undefined ? [] : value.reports, maxObservations, HARD_MAX_OBSERVATIONS);
    for (const [, revision] of revisions) if (typeof revision !== 'bigint' || revision < 0n || revision > 0xffffffffffffffffn) throw new TypeError('Revisions must be unsigned 64-bit BigInts');
    const handles = observed.map(([, report]) => {
      const handle = reports.get(report);
      if (handle === undefined) throw new TypeError('Capability reports must come from Machine.evaluateCapability');
      return handle;
    });
    const call = new Call(this.#table);
    const reportWire = observed.map(([address, report], index) => ({ address: call.address(address, same(address.instance, report.instance) ? index : null), report: index }));
    const spec = { max_observations: representable(maxObservations) ?? 0,
      revisions: revisions.map(([address, revision]) => ({ address: call.address(address), revision: revision.toString() })),
      reports: reportWire, extra: call.extra };
    let executable;
    try { executable = this.#native.snapshot(JSON.stringify(spec), handles); }
    catch (error) { throw failure(error, call); }
    return new CapabilitySnapshot(TOKEN, this, executable, revisions, observed, maxObservations);
  }
  /** Route one source's outputs in order, without dispatching any destination. */
  routeBatch(source, outputs, options = {}) {
    const call = new Call(this.#table);
    const index = call.address(source);
    const batch = read(outputs, this.maxBatch, HARD_MAX_BATCH);
    for (const output of batch) if (output === null || typeof output !== 'object' || !Object.hasOwn(output, 'kind')) throw new TypeError('outputs must be output intents');
    const ids = call.ids(index);
    const spec = { source: index, outputs: batch.map((output, position) => ({ event: find(ids, 'event', output.kind), payload: position })), extra: call.extra };
    let raw;
    try { raw = JSON.parse(this.#native.routeBatch(new native.NativeArena(Math.max(1, batch.length)), JSON.stringify(spec), this.#snapshot(options.capabilities))); }
    catch (error) { throw failure(error, call, { outputs: batch }); }
    return Object.freeze(raw.map((item, position) => this.#item(item, batch[position], source)));
  }
  #item(item, output, source) {
    if (item.kind === 'unmatched') return Object.freeze({ kind: 'unmatched', source, intent: output });
    const destination = this.#table.addresses[item.destination];
    const ids = required(this.#table.ids[item.destination]);
    if (item.kind === 'routed') return Object.freeze({ kind: 'routed', source, destination,
      event: Object.freeze({ kind: ids.resolve('event', item.event), payload: output.payload }) });
    return Object.freeze({ kind: 'deferred', source, destination, capability: ids.resolve('capability', item.capability), status: item.status, intent: output });
  }
  /** Take exclusive use of every registered machine until the binding is disposed. */
  bind(instances) {
    const entries = read(instances instanceof Map ? instances.entries() : instances, 2 * HARD_MAX_INSTANCES, 2 * HARD_MAX_INSTANCES);
    for (const entry of entries) {
      if (!Array.isArray(entry) || entry.length !== 2 || !(entry[0] instanceof InstanceAddress) || !(entry[1] instanceof Machine)) throw new TypeError('Bindings must be [InstanceAddress, Machine] pairs');
    }
    return new BoundComposition(TOKEN, this, entries);
  }
}
/** Exclusive use of bound machines, like Rust's `&mut` borrows. */
class BoundComposition {
  #composition; #addresses; #machines; #contexts; #busy = false;
  constructor(token, composition, entries) {
    if (token !== TOKEN) throw new TypeError('Bind through Composition.bind');
    const { native: executable, table } = composition._internal(TOKEN);
    const call = new Call(table);
    const addresses = entries.map(([address]) => call.address(address));
    const machines = entries.map(([, machine]) => machine);
    const owned = machines.map(context);
    for (const value of new Set(owned)) {
      if (value.busy) throw new errors.BusyError('An operation is already active on this instance');
      if (value.owner !== undefined) throw new errors.BusyError('The machine is already bound to a composition');
    }
    try { executable.bind(JSON.stringify({ extra: call.extra, addresses }), owned.map(value => value.native)); }
    catch (error) { throw failure(error, call); }
    for (const value of owned) value.owner = this;
    this.#composition = composition; this.#addresses = addresses; this.#machines = machines; this.#contexts = owned;
    markAsUncloneable(this);
  }
  /** Dispatch inputs and route accepted outputs FIFO until quiescence or a budget stop. */
  propagate(inputs, options) {
    if (options === null || typeof options !== 'object' || !(options.limits instanceof PropagationLimits)) throw new TypeError('propagate requires { limits: PropagationLimits }');
    if (this.#busy) throw new errors.BusyError('This composition is already propagating');
    if (this.#composition === undefined) throw new errors.DisposedError('The composition binding has been disposed');
    this.#busy = true;
    try { return this.#propagate(inputs, options); }
    finally { this.#busy = false; }
  }
  #propagate(inputs, { limits, facts, capabilities }) {
    const { native: executable, table } = this.#composition._internal(TOKEN);
    const deliveries = read(inputs, limits.maxIntents, limits.maxIntents);
    if (!deliveries.every(delivery => delivery instanceof Delivery)) throw new TypeError('inputs must be Delivery values');
    const call = new Call(table);
    const lowered = deliveries.map((delivery, position) => {
      const index = call.address(delivery.destination);
      return { destination: index, event: find(call.ids(index), 'event', delivery.event.kind), payload: position + 1 };
    });
    const arena = new native.NativeArena(deliveries.length + 1);
    const scratch = { values: new Map([[0, facts], ...deliveries.map((delivery, position) => [position + 1, delivery.event.payload])]) };
    const spec = { binding: { extra: call.extra, addresses: this.#addresses }, inputs: lowered, facts: 0,
      max_steps: limits.maxSteps, max_intents: limits.maxIntents };
    const sources = new Map(this.#contexts.map(value => [value.arenaId, value]));
    sources.set(arena.arenaId(), scratch);
    const snapshot = this.#composition._snapshot(TOKEN, capabilities);
    const run = index => {
      if (index < this.#contexts.length) return this.#contexts[index].operation(() => run(index + 1), false, sources, this);
      let result;
      try {
        result = executable.propagate(arena, JSON.stringify(spec), this.#contexts.map(value => value.native),
          this.#contexts.map(value => value.invoke), snapshot);
      } catch (error) { throw failure(error, call); }
      return report(table, sources, deliveries, JSON.parse(result.report), result.failure);
    };
    return run(0);
  }
  /** Release exclusive use; idempotent. */
  dispose() {
    if (this.#busy) throw new errors.BusyError('This composition is propagating');
    for (const value of this.#contexts ?? []) if (value.owner === this) value.owner = undefined;
    this.#contexts = []; this.#composition = undefined;
  }
  [Symbol.dispose]() { this.dispose(); }
}
function report(table, sources, deliveries, raw, nativeFailure) {
  const value = key => sources.get(key.arena).values.get(key.key);
  const steps = raw.steps.map(step => Object.freeze({ destination: table.addresses[step.destination],
    outcome: decodeOutcome(step.outcome, required(table.ids[step.destination]), value,
      step.input === null ? undefined : { kind: deliveries[step.input].event.kind }) }));
  const effects = raw.effects.map(effect => Object.freeze({ source: table.addresses[effect.source],
    intent: Object.freeze({ id: required(table.ids[effect.source]).resolve('effect', effect.id), payload: value(effect.payload) }) }));
  const unrouted = raw.unrouted.map(item => {
    const source = table.addresses[item.source];
    const intent = Object.freeze({ kind: required(table.ids[item.source]).resolve('event', item.event), payload: value(item.payload) });
    if (item.kind === 'unmatched') return Object.freeze({ kind: 'unmatched', source, intent });
    return Object.freeze({ kind: 'deferred', source, destination: table.addresses[item.destination],
      capability: required(table.ids[item.destination]).resolve('capability', item.capability), status: item.status, intent });
  });
  const pending = raw.pending.map(item => item.input !== null ? deliveries[item.input] : new Delivery(table.addresses[item.destination],
    { kind: required(table.ids[item.destination]).resolve('event', item.event), payload: value(item.payload) }));
  const result = { status: raw.status, steps: Object.freeze(steps), effects: Object.freeze(effects), unrouted: Object.freeze(unrouted),
    pending: Object.freeze(pending) };
  if (nativeFailure !== undefined && nativeFailure !== null) result.error = errors.exception(nativeFailure, table.ids[table.find(pending[0].destination)]);
  return Object.freeze(result);
}
module.exports = { Composition, BoundComposition, CapabilitySnapshot };
