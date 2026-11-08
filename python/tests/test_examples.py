"""Execute the documented native example through an independent interpreter."""
import asyncio
from pathlib import PurePath
import sys
import unittest

class ExampleTests(unittest.IsolatedAsyncioTestCase):
    async def test_order_example_runs_and_emits_mermaid(self):
        example = PurePath(__file__).parent.parent / 'examples' / 'order.py'
        process = await asyncio.create_subprocess_exec(sys.executable, str(example),
            stdin=asyncio.subprocess.DEVNULL, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
        try:
            # bounded: this controlled two-state example emits one small diagram.
            stdout, stderr = await asyncio.wait_for(process.communicate(), timeout=10)
        finally:
            if process.returncode is None:
                process.kill()
                await process.wait()
        self.assertEqual(process.returncode, 0, stderr.decode())
        self.assertTrue(stdout.startswith(b'stateDiagram-v2\n'))
        self.assertIn(b's0 --> s1 : Approve order / Approve [guard: Positive total]', stdout)
