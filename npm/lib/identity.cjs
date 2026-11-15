'use strict';
// Role-aware identity codecs. An IdentityMap is the Rust IdentityMap<T, R> for
// one identity role: bounded (1..4096), one-to-one and explicit. The kernel
// validates the entries; JavaScript only interns values with Map semantics, so
// object tokens compare by identity and nothing is stringified.
const { markAsUncloneable } = require('node:worker_threads');
const native = require('./native.cjs');
const errors = require('./errors.cjs');
const MAX_ENTRIES = 4096;
const IdentityRole = Object.freeze({ Definition: 'definition', Instance: 'instance', State: 'state', Event: 'event',
  Transition: 'transition', Action: 'action', Condition: 'condition', Effect: 'effect', Disposition: 'disposition',
  Capability: 'capability', Operation: 'operation', Scenario: 'scenario', Assertion: 'assertion', Scope: 'scope',
  Story: 'story', Implementation: 'implementation' });
const roles = new Set(Object.values(IdentityRole));
const failures = Object.freeze({ invalid_limit: errors.IdentityMapLimitError, limit_exceeded: errors.IdentityMapLimitError,
  duplicate_identity: errors.DuplicateIdentityError, duplicate_representation: errors.DuplicateRepresentationError,
  unknown_identity: errors.UnknownIdentityError, unknown_representation: errors.UnknownRepresentationError });
function checkedRole(value) {
  if (!roles.has(value)) throw new errors.IdentityRoleError('role must be an IdentityRole value');
  return value;
}
class IdentityMap {
  #role; #maxEntries; #entries; #identities; #representations; #native;
  constructor(options) {
    if (options === null || typeof options !== 'object') throw new errors.IdentityMapError('IdentityMap requires an options object');
    for (const key of Reflect.ownKeys(options)) {
      if (!['role', 'entries', 'maxEntries'].includes(key)) throw new errors.IdentityMapError(`Unknown IdentityMap option ${String(key)}`);
    }
    const role = checkedRole(options.role);
    const maxEntries = options.maxEntries === undefined ? MAX_ENTRIES : options.maxEntries;
    if (!Number.isInteger(maxEntries) || maxEntries < 0 || maxEntries > 0xffffffff) {
      throw new errors.IdentityMapLimitError('maxEntries must be an integer between 1 and 4096', { role });
    }
    // Consume at most one entry past the bound; the kernel decides validity.
    const budget = maxEntries >= 1 && maxEntries <= MAX_ENTRIES ? maxEntries + 1 : 0;
    const entries = []; const identities = new Map(); const representations = new Map();
    const identityIndexes = []; const representationIndexes = [];
    if (budget > 0) {
      for (const entry of options.entries) {
        if (entries.length === budget) break;
        if (!Array.isArray(entry) || entry.length !== 2) {
          throw new errors.IdentityMapError('IdentityMap entries must be [identity, representation] pairs', { role, entry: entries.length });
        }
        const [identity, representation] = entry;
        if (!identities.has(identity)) identities.set(identity, entries.length);
        if (!representations.has(representation)) representations.set(representation, entries.length);
        identityIndexes.push(identities.get(identity)); representationIndexes.push(representations.get(representation));
        entries.push(Object.freeze([identity, representation]));
      }
    }
    this.#role = role; this.#maxEntries = maxEntries;
    try { this.#native = new native.NativeIdentityMap(identityIndexes, representationIndexes, maxEntries); }
    catch (error) { this.#raise(error, entries); }
    this.#entries = Object.freeze(entries); this.#identities = identities; this.#representations = representations;
    // A structured clone would silently drop the private kernel codec.
    markAsUncloneable(this);
    Object.freeze(this);
  }
  get role() { return this.#role; }
  get maxEntries() { return this.#maxEntries; }
  get size() { return this.#entries.length; }
  /** Registered [identity, representation] pairs in declaration order. */
  [Symbol.iterator]() { return this.#entries[Symbol.iterator](); }
  encode(identity) {
    const index = this.#identities.get(identity);
    if (index === undefined) throw new errors.UnknownIdentityError('unknown mapped identity', { role: this.#role, identity });
    try { return this.#entries[this.#native.encode(index)][1]; }
    catch (error) { this.#raise(error, this.#entries); }
  }
  decode(representation) {
    const index = this.#representations.get(representation);
    if (index === undefined) throw new errors.UnknownRepresentationError('unknown identity representation', { role: this.#role, representation });
    try { return this.#entries[this.#native.decode(index)][0]; }
    catch (error) { this.#raise(error, this.#entries); }
  }
  #raise(error, entries) {
    if (typeof error.message !== 'string' || !error.message.startsWith('{')) throw error;
    const info = JSON.parse(error.message);
    const Type = info.code === 'identity_map' ? failures[info.reason] : undefined;
    if (Type === undefined) throw new Error('Unrecognized native identity map failure', { cause: error });
    const context = { role: this.#role };
    if (info.limit !== null) context.limit = info.limit;
    if (info.entry !== null) {
      context.entry = info.entry;
      if (Type === errors.DuplicateIdentityError) context.identity = entries[info.entry][0];
      if (Type === errors.DuplicateRepresentationError) context.representation = entries[info.entry][1];
    }
    throw new Type(info.message, context);
  }
}
class MachineIdentityMaps {
  #maps;
  constructor(maps) {
    if (maps === null || typeof maps !== 'object') throw new errors.IdentityRoleError('MachineIdentityMaps requires an object keyed by role');
    const result = new Map();
    for (const key of Reflect.ownKeys(maps)) {
      const role = checkedRole(key);
      const value = maps[key];
      if (value === undefined) continue;
      if (!(value instanceof IdentityMap)) throw new errors.IdentityRoleError(`${role} must be an IdentityMap`, { role });
      if (value.role !== role) throw new errors.IdentityRoleError(`${value.role} codec supplied for the ${role} role`, { role, actual: value.role });
      result.set(role, value);
    }
    this.#maps = result;
    markAsUncloneable(this);
    Object.freeze(this);
  }
  /** Return the codec for role; an unconfigured role is an error. */
  map(role) {
    const value = this.#maps.get(checkedRole(role));
    if (value === undefined) throw new errors.IdentityRoleError(`no codec is configured for the ${role} role`, { role });
    return value;
  }
  encode(role, identity) { return this.map(role).encode(identity); }
  decode(role, representation) { return this.map(role).decode(representation); }
}
module.exports = { IdentityRole, IdentityMap, MachineIdentityMaps };
