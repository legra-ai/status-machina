'use strict';
// Each call owns its memo. Symbols retain immutable identity; custom prototypes
// require an explicit application snapshot instead of being erased.
function snapshot(value, memo = new Map()) {
  if (value === null || (typeof value !== 'object' && typeof value !== 'function')) return value;
  if (typeof value === 'function') throw new TypeError('Function-valued data needs an explicit snapshot function');
  if (memo.has(value)) return memo.get(value);
  const prototype = Object.getPrototypeOf(value);
  let clone;
  let indexed = false;
  if (prototype === Object.prototype || prototype === null) clone = Object.create(prototype);
  else if (prototype === Array.prototype) clone = [];
  else if (prototype === Date.prototype) clone = new Date(value.getTime());
  else if (prototype === RegExp.prototype) clone = new RegExp(value.source, value.flags);
  else if (prototype === Map.prototype) clone = new Map();
  else if (prototype === Set.prototype) clone = new Set();
  else if (prototype === ArrayBuffer.prototype) clone = value.slice(0);
  else if (Buffer.isBuffer(value) && prototype === Buffer.prototype) {
    clone = Buffer.from(snapshot(value.buffer, memo), value.byteOffset, value.length);
    indexed = true;
  } else if (ArrayBuffer.isView(value)) {
    const Constructor = [DataView, Int8Array, Uint8Array, Uint8ClampedArray, Int16Array,
      Uint16Array, Int32Array, Uint32Array, Float32Array, Float64Array, BigInt64Array, BigUint64Array]
      .find(Type => prototype === Type.prototype);
    if (Constructor === undefined) throw new TypeError('Custom data classes need an explicit snapshot function');
    const buffer = snapshot(value.buffer, memo);
    clone = Constructor === DataView ? new DataView(buffer, value.byteOffset, value.byteLength)
      : new Constructor(buffer, value.byteOffset, value.length);
    indexed = true;
  } else throw new TypeError('Custom data classes need an explicit snapshot function; their prototype will not be erased');
  memo.set(value, clone);
  if (prototype === Map.prototype) {
    for (const [key, item] of value) clone.set(snapshot(key, memo), snapshot(item, memo));
  } else if (prototype === Set.prototype) {
    for (const item of value) clone.add(snapshot(item, memo));
  }
  for (const key of Reflect.ownKeys(value)) {
    if (indexed && typeof key === 'string' && /^(0|[1-9][0-9]*)$/.test(key)) continue;
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!Object.hasOwn(descriptor, 'value')) throw new TypeError('Accessor-valued data needs an explicit snapshot function');
    Object.defineProperty(clone, key, { ...descriptor, value: snapshot(descriptor.value, memo) });
  }
  if (!Object.isExtensible(value)) Object.preventExtensions(clone);
  return clone;
}
module.exports = { snapshot };
