'use strict';
// Composition values: addresses, routes, registries, deliveries and budgets.
// Identity comparisons use Map semantics (SameValueZero), component by component.
const native = require('./native.cjs');
const { Machine } = require('./machine.cjs');
const { PropagationLimitsError, isNative } = require('./errors.cjs');
// Kernel ceilings: they bound how many native entries are read, never what is accepted.
const HARD_MAX_INSTANCES = 512;
const HARD_MAX_ROUTES = 512;
function same(left, right) { return left === right || (left !== left && right !== right); }
/** A bound the kernel can receive; anything else reaches it as absent or zero. */
function representable(value) { return Number.isSafeInteger(value) && value >= 0 && value <= 0xffffffff ? value : undefined; }
/** Read at most one entry beyond the declared bound, so the kernel decides the bound. */
function read(values, limit, ceiling) {
  const bound = Number.isInteger(limit) && limit > 0 ? Math.min(limit, ceiling) : 0;
  const result = [];
  for (const value of values) {
    result.push(value);
    if (result.length > bound) break;
  }
  return Object.freeze(result);
}
function options(value, allowed, label) {
  if (value === null || typeof value !== 'object') throw new TypeError(`${label} must be an options object`);
  for (const key of Reflect.ownKeys(value)) if (!allowed.includes(key)) throw new TypeError(`Unknown ${label} field ${String(key)}`);
}
function integer(value, label) {
  if (!Number.isInteger(value)) throw new TypeError(`${label} must be an integer`);
  return value;
}
/** A caller-owned address: one instance identity within one routing scope. */
class InstanceAddress {
  constructor(scope, instance) {
    this.scope = scope; this.instance = instance;
    Object.freeze(this);
  }
  /** Component-wise Map semantics; object tokens compare by identity. */
  equals(other) { return other instanceof InstanceAddress && same(this.scope, other.scope) && same(this.instance, other.instance); }
}
function address(value, label) {
  if (!(value instanceof InstanceAddress)) throw new TypeError(`${label} must be an InstanceAddress`);
  return value;
}
/** Deliver `event` emitted at `source` to `destination` as `inputEvent`. */
class Route {
  constructor(value) {
    options(value, ['source', 'event', 'destination', 'inputEvent', 'capability'], 'route');
    for (const key of ['event', 'inputEvent']) if (!Object.hasOwn(value, key)) throw new TypeError(`route requires ${key}`);
    this.source = address(value.source, 'route source'); this.event = value.event;
    this.destination = address(value.destination, 'route destination'); this.inputEvent = value.inputEvent;
    this.capability = value.capability;
    Object.freeze(this);
  }
}
function pairs(values, label) {
  for (const entry of values) {
    if (!Array.isArray(entry) || entry.length !== 2 || !(entry[0] instanceof InstanceAddress) || !(entry[1] instanceof Machine)) {
      throw new TypeError(`${label} must be [InstanceAddress, Machine] pairs`);
    }
  }
  return values;
}
/** Bounded [address, machine] registrations; the composition retains no machine. */
class InstanceRegistry {
  constructor(value = {}) {
    options(value, ['entries', 'maxInstances'], 'registry');
    this.maxInstances = integer(value.maxInstances === undefined ? 128 : value.maxInstances, 'maxInstances');
    this.entries = pairs(read(value.entries === undefined ? [] : value.entries, this.maxInstances, HARD_MAX_INSTANCES), 'registry entries');
    Object.freeze(this);
  }
}
/** An input addressed to exactly one registered instance. */
class Delivery {
  constructor(destination, event) {
    if (event === null || typeof event !== 'object' || !Object.hasOwn(event, 'kind')) throw new TypeError('A delivery needs an event with a kind');
    this.destination = address(destination, 'delivery destination');
    this.event = Object.freeze({ kind: event.kind, payload: event.payload });
    Object.freeze(this);
  }
}
/** Explicit work budgets for one propagate call, each within 1..4096. */
class PropagationLimits {
  constructor(value) {
    options(value, ['maxSteps', 'maxIntents'], 'propagation limits');
    this.maxSteps = integer(value.maxSteps, 'maxSteps'); this.maxIntents = integer(value.maxIntents, 'maxIntents');
    try { native.propagationLimits(representable(this.maxSteps), representable(this.maxIntents)); }
    catch (error) {
      if (!isNative(error)) throw error;
      throw new PropagationLimitsError(JSON.parse(error.message).message,
        { reason: 'invalid_limits', maxSteps: this.maxSteps, maxIntents: this.maxIntents });
    }
    Object.freeze(this);
  }
}
const PropagationStatus = Object.freeze({ Quiescent: 'quiescent', StepLimit: 'step_limit', IntentLimit: 'intent_limit',
  UnboundDestination: 'unbound_destination', DispatchFailed: 'dispatch_failed' });
module.exports = { InstanceAddress, Route, InstanceRegistry, Delivery, PropagationLimits, PropagationStatus,
  HARD_MAX_INSTANCES, HARD_MAX_ROUTES, read, same, representable };
