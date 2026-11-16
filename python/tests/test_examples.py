"""Execute the documented native example through an independent interpreter."""
import asyncio
import io
from pathlib import PurePath
import sys
import unittest

class ExampleTests(unittest.IsolatedAsyncioTestCase):
    async def run_example(self, name):
        example = PurePath(__file__).parent.parent / 'examples' / name
        process = await asyncio.create_subprocess_exec(sys.executable, str(example),
            stdin=asyncio.subprocess.DEVNULL, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
        try:
            # bounded: these controlled examples print a few small lines.
            stdout, stderr = await asyncio.wait_for(process.communicate(), timeout=10)
        finally:
            if process.returncode is None:
                process.kill()
                await process.wait()
        self.assertEqual(process.returncode, 0, stderr.decode())
        # print() writes a text stream; Windows translates its newlines to CRLF.
        return io.TextIOWrapper(io.BytesIO(stdout), encoding='utf-8').read()

    async def test_order_example_runs_and_emits_mermaid(self):
        diagram = await self.run_example('order.py')
        self.assertTrue(diagram.startswith('stateDiagram-v2\n'))
        self.assertIn('s0 --> s1 : Approve order / Approve [guard: Positive total]', diagram)

    async def test_composition_example_propagates_across_three_scopes(self):
        output = await self.run_example('composition.py')
        self.assertEqual(output.splitlines(), [
            'sales: OPEN -> PAID', 'warehouse: WAITING -> PACKED', 'finance: DRAFT -> ISSUED',
            'warehouse effect: label for order-7', 'QUIESCENT Ledger(total=120)'])

    async def test_retry_example_consumes_each_wake_once(self):
        output = await self.run_example('retry.py')
        self.assertEqual(output.splitlines(), [
            'wait timer-1 until 10:00', 'wake: transitioned', 'duplicate: ignored',
            'wait timer-2 until 12:00', 'wake: transitioned', 'duplicate: ignored', 'attempts: 3'])
