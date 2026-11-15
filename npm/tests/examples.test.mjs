import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
const execute = promisify(execFile);
test('documented native order example runs and emits Mermaid in a separate Node process', async () => {
  const example = fileURLToPath(new URL('../examples/order.mjs', import.meta.url));
  const { stdout } = await execute(process.execPath, [example], { timeout: 10000, maxBuffer: 8192 });
  assert.ok(stdout.startsWith('stateDiagram-v2\n'));
  assert.ok(stdout.includes('s0 --> s1 : Approve order / Approve [guard: Positive total]'));
});
test('documented composition example propagates across three scopes in a separate Node process', async () => {
  const example = fileURLToPath(new URL('../examples/composition.mjs', import.meta.url));
  const { stdout } = await execute(process.execPath, [example], { timeout: 10000, maxBuffer: 8192 });
  assert.deepEqual(stdout.split(/\r?\n/).filter(Boolean), ['sales: OPEN -> PAID', 'warehouse: WAITING -> PACKED',
    'finance: DRAFT -> ISSUED', 'warehouse effect: label for order-7', 'quiescent {"total":120}']);
});
