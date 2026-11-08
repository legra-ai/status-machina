'use strict';
const { InspectionError } = require('./errors.cjs');
const roles = Object.freeze({ states: 'state', transitions: 'transition', events: 'event', conditions: 'condition', dispositions: 'disposition' });
function limits(options = {}) {
  if (options === null || typeof options !== 'object') throw new InspectionError('Inspection limits must be an object');
  for (const key of Reflect.ownKeys(options)) {
    if (key !== 'maxEntries' && key !== 'maxOutputBytes') throw new InspectionError('Unknown inspection limit');
  }
  const result = { maxEntries: 4096, maxOutputBytes: 1048576, ...options };
  for (const [key, maximum] of [['maxEntries', 1000000], ['maxOutputBytes', 1048576]]) {
    if (!Number.isInteger(result[key]) || result[key] < 1 || result[key] > maximum) {
      throw new InspectionError(`${key} must be an integer between 1 and ${maximum}`);
    }
  }
  return result;
}
function labels(source, inventory, ids, limits) {
  if (source === null || typeof source !== 'object') throw new InspectionError('Diagram labels must be an object of Maps');
  for (const key of Reflect.ownKeys(source)) if (!Object.hasOwn(roles, key)) throw new InspectionError('Unknown diagram label role');
  const count = inventory.states.length + inventory.transitions.length + inventory.dispositions.length;
  if (count > limits.maxEntries) throw new InspectionError('Graph inventory exceeds maxEntries', { limit: limits.maxEntries });
  let remaining = limits.maxOutputBytes;
  const result = {};
  for (const [name, role] of Object.entries(roles)) {
    const mapping = source[name]; const values = [];
    // bounded: only labels requested by the validated graph inventory are copied.
    for (const index of inventory[name]) {
      const identifier = ids.resolve(role, index);
      let present;
      try { present = mapping !== undefined && Map.prototype.has.call(mapping, identifier); }
      catch (cause) { throw new InspectionError('Diagram label roles must contain Maps', { role }, { cause }); }
      if (!present) throw new InspectionError('Missing diagram label', { role, identifier });
      const text = Map.prototype.get.call(mapping, identifier);
      if (typeof text !== 'string') throw new InspectionError('Diagram labels must be strings', { role, identifier });
      if (text.length > remaining) throw new InspectionError('Label UTF-8 exceeds maxOutputBytes', { limit: limits.maxOutputBytes });
      for (const character of text) {
        const code = character.codePointAt(0);
        if (code >= 0xd800 && code <= 0xdfff) throw new InspectionError('Diagram labels must contain valid Unicode', { role, identifier });
      }
      remaining -= Buffer.byteLength(text, 'utf8');
      if (remaining < 0) throw new InspectionError('Label UTF-8 exceeds maxOutputBytes', { limit: limits.maxOutputBytes });
      values.push({ id: index, text });
    }
    result[name] = values;
  }
  return JSON.stringify(result);
}
function coverage(raw, ids) {
  return Object.freeze({ states: Object.freeze(raw.states.map(state => Object.freeze({ ...state, id: ids.resolve('state', state.id) }))),
    transitions: Object.freeze(raw.transitions.map(id => ids.resolve('transition', id))),
    dispositions: Object.freeze(raw.dispositions.map(id => ids.resolve('disposition', id))) });
}
module.exports = { limits, labels, coverage };
