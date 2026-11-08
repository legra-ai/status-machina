'use strict';
const { BusyError, DisposedError, LimitError, translate } = require('./errors.cjs');
class Context {
  constructor(definition, identity) {
    const internals = definition._internal();
    this.definition = definition; this.ids = internals.compiler.ids; this.snapshot = internals.snapshot;
    this.inputSnapshot = internals.inputSnapshot; this.identity = identity; this.capacity = internals.limits.maxValues;
    this.values = new Map(); this.nextKey = 0; this.freeKeys = []; this.arenaId = undefined; this.native = undefined;
    this.closed = false; this.busy = false; this.sources = undefined;
    this.invoke = encoded => this.callback(encoded);
  }
  allocate(value) {
    if (this.values.size === this.capacity) throw new LimitError('Native value arena is full');
    if (this.freeKeys.length === 0 && this.nextKey === 2 ** 32) throw new LimitError('Native value key space is exhausted');
    const key = this.freeKeys.length > 0 ? this.freeKeys.pop() : this.nextKey++; this.values.set(key, value); return key;
  }
  sweep() {
    const live = new Set(this.native === undefined ? [] : this.native.liveKeys());
    for (const key of this.values.keys()) if (!live.has(key)) { this.values.delete(key); this.freeKeys.push(key); }
  }
  resolve(reference) {
    if (this.arenaId === undefined) this.arenaId = reference.arena;
    let context;
    if (reference.arena === this.arenaId) context = this;
    else if (this.sources !== undefined && this.sources.has(reference.arena)) context = this.sources.get(reference.arena);
    else throw new Error('Native value belongs to an unbound arena');
    if (!context.values.has(reference.key)) throw new Error('Native value lease is missing');
    return context.values.get(reference.key);
  }
  operation(body, allowClosed = false, sources) {
    if (this.busy) throw new BusyError('An operation is already active on this instance');
    if (this.closed && !allowClosed) throw new DisposedError('The machine has been disposed');
    this.busy = true;
    try {
      this.sweep(); this.sources = sources;
      return body(this);
    } catch (error) { translate(error, this.ids); }
    finally { this.sources = undefined; try { this.sweep(); } finally { this.busy = false; } }
  }
  detach() {
    this.closed = true; this.definition = undefined; this.ids = undefined;
    this.snapshot = undefined; this.inputSnapshot = undefined; this.identity = undefined;
  }
  copy(value, input = false) {
    const result = (input ? this.inputSnapshot : this.snapshot)(value);
    if (result !== null && result !== undefined && typeof result.then === 'function') throw new TypeError('Snapshot callbacks must return synchronously, not a Promise');
    return result;
  }
  callback(encoded) {
    const request = JSON.parse(encoded);
    const values = request.values.map(key => this.resolve(key));
    if (request.kind === 'snapshot') return JSON.stringify({ kind: 'value', value: this.allocate(this.copy(values[0])) });
    const entry = this.definition._internal().callbacks[request.callback];
    let args;
    if (request.kind === 'invariant') args = [this.copy(values[0])];
    else if (['predicate', 'construct', 'action'].includes(request.kind)) args = [request.kind === 'action' ? values[0] : this.copy(values[0]), this.copy(values[1], true), this.copy(values[2], true)];
    else throw new Error('Unsupported native callback request');
    const result = entry.function(...args);
    if (result !== null && result !== undefined && typeof result.then === 'function') throw new TypeError('Machine callbacks must return synchronously, not a Promise');
    let response;
    if (request.kind === 'predicate' || request.kind === 'invariant') {
      if (typeof result !== 'boolean') throw new TypeError('Predicates and invariants must return boolean');
      response = { kind: 'bool', value: result };
    } else if (request.kind === 'action' && !entry.update) {
      if (result !== undefined) throw new TypeError('An apply action must return undefined; use update to replace data');
      response = { kind: 'unit' };
    } else response = { kind: 'value', value: this.allocate(result) };
    return JSON.stringify(response);
  }
}
module.exports = { Context };
