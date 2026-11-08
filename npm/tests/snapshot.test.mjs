import assert from 'node:assert/strict';
import test from 'node:test';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { snapshot } = require('../lib/snapshot.cjs');

test('native typed views preserve cycles through their backing buffer', () => {
  const view = new Uint8Array([1, 2]);
  view.buffer.owner = view;
  const copy = snapshot(view);
  assert.notEqual(copy.buffer, view.buffer);
  assert.equal(copy.buffer.owner, copy);
  copy[0] = 9;
  assert.equal(view[0], 1);
});

test('native symbols, Buffer, Map and shared aliases survive isolated snapshots', () => {
  const id = Symbol('domain identity');
  const buffer = Buffer.from([1, 2]);
  const original = { id, [id]: buffer, buffer, again: buffer, map: new Map([[id, buffer]]) };
  const copy = snapshot(original);
  assert.equal(copy.id, id);
  assert.ok(Buffer.isBuffer(copy.buffer));
  assert.equal(copy[id], copy.buffer);
  assert.equal(copy.buffer, copy.again);
  assert.equal(copy.map.get(id), copy.buffer);
  copy.buffer[0] = 9;
  assert.equal(buffer[0], 1);
});
