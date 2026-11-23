#!/usr/bin/env python3
"""Run unittest test suites in parallel across multiple processes.

Executes each discovered test module via `python3 -m unittest discover -s tests -p <pattern>`
in an isolated subprocess, ensuring complete separation of sys.path, sys.modules and mocks
while aggregating outputs and test counts into a deterministic final summary.
"""

import glob
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def discover_patterns(tests_dir: Path) -> list[str]:
    """Return sorted filenames of all test suites under the given directory."""
    files = sorted(glob.glob(str(tests_dir / "test_*.py")))
    return [os.path.basename(f) for f in files]


def run_suite(pattern: str, tests_dir: str, verbose: bool = False) -> tuple[str, int, str, str, float]:
    """Execute one test pattern in an isolated process to guarantee clean environment boundaries."""
    cmd = [sys.executable, "-m", "unittest", "discover", "-s", tests_dir, "-p", pattern]
    if verbose:
        cmd.append("-v")
    t0 = time.perf_counter()
    res = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, errors="replace")
    elapsed = time.perf_counter() - t0
    return pattern, res.returncode, res.stdout, res.stderr, elapsed


def parse_summary(stderr: str) -> tuple[int, int, int]:
    """Extract (tests_run, failures_or_errors, skipped) from a unittest stderr stream."""
    ran_match = re.search(r"^Ran (\d+) tests?", stderr, re.M)
    ran = int(ran_match.group(1)) if ran_match else 0
    failures = 0
    skipped = 0
    fail_match = re.search(r"FAILED \((?:failures=(\d+))?(?:, )?(?:errors=(\d+))?(?:, )?(?:skipped=(\d+))?\)", stderr)
    if fail_match:
        failures += int(fail_match.group(1) or 0) + int(fail_match.group(2) or 0)
        skipped += int(fail_match.group(3) or 0)
    else:
        ok_match = re.search(r"OK \((?:skipped=(\d+))\)", stderr)
        if ok_match:
            skipped += int(ok_match.group(1))
    return ran, failures, skipped


def parse_args() -> tuple[list[str], str, bool]:
    """Parse CLI arguments for pattern, test directory, and verbosity."""
    args = sys.argv[1:]
    verbose = "-v" in args
    tests_dir = "tests"
    explicit_pattern = None

    i = 0
    while i < len(args):
        arg = args[i]
        if arg == "-p" and i + 1 < len(args):
            explicit_pattern = args[i + 1]
            i += 2
        elif arg == "-s" and i + 1 < len(args):
            tests_dir = args[i + 1]
            i += 2
        else:
            i += 1

    if explicit_pattern:
        patterns = [explicit_pattern]
    else:
        patterns = discover_patterns(ROOT / tests_dir)

    return patterns, tests_dir, verbose


def main() -> int:
    patterns, tests_dir, verbose = parse_args()
    if not patterns:
        print("No tests discovered.", file=sys.stderr)
        return 0

    max_workers = min(os.cpu_count() or 4, 8)
    started = time.perf_counter()

    with ThreadPoolExecutor(max_workers=max_workers) as pool:
        results = list(pool.map(lambda pat: run_suite(pat, tests_dir, verbose), patterns))

    total_ran = 0
    total_failures = 0
    total_skipped = 0
    has_error = False

    # Print any failures or verbose outputs in deterministic command order
    for pattern, returncode, stdout, stderr, elapsed in results:
        ran, failures, skipped = parse_summary(stderr)
        total_ran += ran
        total_failures += failures
        total_skipped += skipped
        if returncode != 0:
            has_error = True
            if stdout:
                sys.stdout.write(stdout)
            if stderr:
                sys.stderr.write(stderr)
        elif verbose and stdout:
            sys.stdout.write(stdout)

    sys.stdout.flush()
    sys.stderr.flush()

    total_time = time.perf_counter() - started
    status_str = f"FAILED (failures={total_failures})" if has_error else "OK"
    if total_skipped and not has_error:
        status_str += f" (skipped={total_skipped})"

    print("-" * 70, file=sys.stderr)
    print(f"Ran {total_ran} tests in {total_time:.3f}s", file=sys.stderr)
    print(f"\n{status_str}", file=sys.stderr)

    return 1 if has_error else 0


if __name__ == "__main__":
    sys.exit(main())
