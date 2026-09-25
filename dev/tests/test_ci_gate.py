"""Execute the actual aggregate gate for success, skip and failure scenarios."""

import os
from pathlib import Path
import shutil
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]


class GateTests(unittest.TestCase):
    def run_gate(self, **overrides):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        block = workflow.rsplit("        run: |\n", 1)[1]
        script = "\n".join(line[10:] for line in block.splitlines())
        env = {**os.environ, "SCOPE_RESULT": "success", "PUBLICATION_RESULT": "success",
               "RUNTIME_REQUIRED": "true", "RUST_RESULT": "success", "STANDALONE_RESULT": "success",
               **overrides}
        return subprocess.run([shutil.which("bash") or "bash", "-e", "-c", script],
                              env=env, capture_output=True).returncode

    def test_full_and_intentional_prose_skip_pass(self):
        self.assertEqual(self.run_gate(), 0)
        self.assertEqual(self.run_gate(RUNTIME_REQUIRED="false", RUST_RESULT="skipped", STANDALONE_RESULT="skipped"), 0)

    def test_required_jobs_cannot_be_failed_cancelled_or_accidentally_skipped(self):
        for key in ("SCOPE_RESULT", "PUBLICATION_RESULT", "RUST_RESULT", "STANDALONE_RESULT"):
            for value in ("failure", "cancelled", "skipped"):
                with self.subTest(key=key, value=value):
                    self.assertNotEqual(self.run_gate(**{key: value}), 0)
        self.assertNotEqual(self.run_gate(RUNTIME_REQUIRED=""), 0)
        self.assertNotEqual(self.run_gate(RUNTIME_REQUIRED="false", RUST_RESULT="failure", STANDALONE_RESULT="skipped"), 0)


if __name__ == "__main__":
    unittest.main()
