"""Tests for tools/ci/parallel_unittest.py."""

import os
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARALLEL_RUNNER = ROOT / "tools" / "ci" / "parallel_unittest.py"


class ParallelUnittestTests(unittest.TestCase):
    def test_runner_file_exists(self):
        self.assertTrue(PARALLEL_RUNNER.is_file())

    def test_run_subsets_passes(self):
        # Run a fast subset of existing tests via parallel_unittest
        res = subprocess.run(
            [sys.executable, str(PARALLEL_RUNNER), "-p", "test_format_scan.py"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0, f"stdout: {res.stdout}\nstderr: {res.stderr}")
        self.assertIn("Ran", res.stderr)
        self.assertIn("OK", res.stderr)

    def test_pattern_discovery(self):
        sys.path.insert(0, str(ROOT / "tools" / "ci"))
        try:
            import parallel_unittest
            patterns = parallel_unittest.discover_patterns(ROOT / "tests")
            self.assertIn("test_pr_ready.py", patterns)
            self.assertTrue(all(p.startswith("test_") and p.endswith(".py") for p in patterns))
        finally:
            if str(ROOT / "tools" / "ci") in sys.path:
                sys.path.remove(str(ROOT / "tools" / "ci"))

    def test_parse_summary(self):
        sys.path.insert(0, str(ROOT / "tools" / "ci"))
        try:
            import parallel_unittest
            ran, failures, skipped = parallel_unittest.parse_summary(
                "Ran 12 tests in 0.05s\n\nOK (skipped=2)"
            )
            self.assertEqual(ran, 12)
            self.assertEqual(failures, 0)
            self.assertEqual(skipped, 2)

            ran, failures, skipped = parallel_unittest.parse_summary(
                "Ran 5 tests in 0.01s\n\nFAILED (failures=2, errors=1)"
            )
            self.assertEqual(ran, 5)
            self.assertEqual(failures, 3)
            self.assertEqual(skipped, 0)
        finally:
            if str(ROOT / "tools" / "ci") in sys.path:
                sys.path.remove(str(ROOT / "tools" / "ci"))


if __name__ == "__main__":
    unittest.main()
