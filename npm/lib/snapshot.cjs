'use strict';
// Objects are allocated before their properties are copied. This preserves
// cycles through typed views and backing buffers without retaining caller aliases.
function snapshot(value) {
  const memo = new Map();
  const pending = [];
  function allocate(original) {
    if (original === null || (typeof original !== 'object' && typeof original !== 'function')) return original;
    if (typeof original === 'function') throw new TypeError('Function-valued data needs an explicit snapshot function');
    if (memo.has(original)) return memo.get(original);
    const prototype = Object.getPrototypeOf(original);
    const descriptors = Object.getOwnPropertyDescriptors(original);
    for (const descriptor of Object.values(descriptors)) {
      if (!Object.hasOwn(descriptor, 'value')) throw new TypeError('Accessor-valued data needs an explicit snapshot function');
    }
    let clone;
    let indexed = false;
    if (prototype === Object.prototype || prototype === null) clone = Object.create(prototype);
    else if (prototype === Array.prototype) clone = [];
    else if (prototype === Date.prototype) clone = new Date(Date.prototype.getTime.call(original));
    else if (prototype === RegExp.prototype) clone = new RegExp(original.source, original.flags);
    else if (prototype === Map.prototype) clone = new Map();
    else if (prototype === Set.prototype) clone = new Set();
    else if (prototype === ArrayBuffer.prototype) clone = ArrayBuffer.prototype.slice.call(original, 0);
    else if (Buffer.isBuffer(original) && prototype === Buffer.prototype) {
      clone = Buffer.from(allocate(original.buffer), original.byteOffset, original.length);
      indexed = true;
    } else if (ArrayBuffer.isView(original)) {
      const Constructor = [DataView, Int8Array, Uint8Array, Uint8ClampedArray, Int16Array,
        Uint16Array, Int32Array, Uint32Array, Float32Array, Float64Array, BigInt64Array, BigUint64Array]
        .find(Type => prototype === Type.prototype);
      if (Constructor === undefined) throw new TypeError('Custom data classes need an explicit snapshot function');
      const buffer = allocate(original.buffer);
      clone = Constructor === DataView ? new DataView(buffer, original.byteOffset, original.byteLength)
        : new Constructor(buffer, original.byteOffset, original.length);
      indexed = true;
    } else throw new TypeError('Custom data classes need an explicit snapshot function; their prototype will not be erased');
    memo.set(original, clone);
    pending.push({ original, clone, prototype, descriptors, indexed });
    return clone;
  }
  const result = allocate(value);
  for (let index = 0; index < pending.length; index++) {
    const { original, clone, prototype, descriptors, indexed } = pending[index];
    if (prototype === Map.prototype) {
      for (const [key, item] of Map.prototype.entries.call(original)) clone.set(allocate(key), allocate(item));
    } else if (prototype === Set.prototype) {
      for (const item of Set.prototype.values.call(original)) clone.add(allocate(item));
    }
    for (const key of Reflect.ownKeys(descriptors)) {
      if (indexed && typeof key === 'string' && /^(0|[1-9][0-9]*)$/.test(key)) continue;
      const descriptor = descriptors[key];
      Object.defineProperty(clone, key, { ...descriptor, value: allocate(descriptor.value) });
    }
    if (!Object.isExtensible(original)) Object.preventExtensions(clone);
  }
  return result;
}
module.exports = { snapshot };
