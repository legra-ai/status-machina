"""Private extraction validation must never publish a public package."""
import pathlib
import unittest


class ReleasePolicyTests(unittest.TestCase):
    def test_publish_requires_public_repository(self):
        workflow = (pathlib.Path(__file__).resolve().parent.parent / '.github/workflows/ci.yml').read_text()
        publish = workflow.split('\n  publish:\n', 1)[1]
        condition = next(line.strip() for line in publish.splitlines() if line.strip().startswith('if:'))
        self.assertIn("github.event.repository.private == false", condition)
        self.assertIn("needs.prepare.outputs.release == 'true'", condition)


if __name__ == '__main__':
    unittest.main()
