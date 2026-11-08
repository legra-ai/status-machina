import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import * as path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
const source = await readFile(new URL('../lib/native.cjs', import.meta.url), 'utf8');
function load(platform, arch) {
  const marker = {}; const loaded = [];
  const context = { __dirname: '/status-machina/lib', module: { exports: undefined },
    process: { platform, arch, versions: { node: '22.0.0' } },
    require: name => {
      if (name === 'node:path') return path;
      loaded.push(name); return marker;
    } };
  vm.runInNewContext(source, context);
  return { value: context.module.exports, marker, loaded };
}
test('macOS native package loads only the ARM64 addon', () => {
  const result = load('darwin', 'arm64');
  assert.equal(result.value, result.marker);
  assert.equal(result.loaded.length, 1);
  assert.ok(result.loaded[0].endsWith('status_machina.darwin-arm64.node'));
  assert.throws(() => load('darwin', 'x64'), /Unsupported Status Machina platform: darwin\/x64/);
});
test('declared macOS artifact targets contain ARM64 only', async () => {
  const manifest = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  assert.deepEqual(manifest.napi.targets.filter(target => target.endsWith('apple-darwin')), ['aarch64-apple-darwin']);
});
