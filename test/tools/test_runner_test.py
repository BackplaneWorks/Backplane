"""Exercise suite dispatch and failure propagation without compiling Bend."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FAKE_TOOL = """#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
with open('calls.jsonl', 'a') as out:
    out.write(json.dumps([Path(sys.argv[0]).name, sys.argv[1:],
                         os.environ.get('BUN_JSC_maxPerThreadStackUsage')]) + '\\n')
if Path(sys.argv[0]).name == 'bun':
    text = Path(sys.argv[1]).read_text()
elif '-o' in sys.argv:
    text = Path(sys.argv[1]).read_text()
    Path(sys.argv[-1]).write_text(text)
    sys.exit(3 if '# emit-error' in text else 0)
else:
    text = Path(sys.argv[1]).read_text()
if '# crash' in text:
    print('runtime crashed')
    sys.exit(2)
elif '# fail' in text:
    print('FAIL regression')
elif '# empty' in text or 'def main(' not in text and Path(sys.argv[0]).name != 'bun':
    print('ALL PROOFS CHECK')
else:
    print('ok regression')
"""


class TestRunnerTest(unittest.TestCase):
    """Keep the positive suite strict across both execution paths."""

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for folder in ("scripts", "test/tools", "bin"):
            (self.root / folder).mkdir(parents=True)
        for name in ("test.sh", "test-shard.py"):
            shutil.copyfile(ROOT / "scripts" / name, self.root / "scripts" / name)
        # This fixture executes only fake tools, not heavy work. Real lock/RAM
        # admission has separate heavy_guard_test.py coverage. Never invoke the
        # host's guard/offload policy from a dispatch unit test.
        (self.root / "scripts/heavy-guard.sh").write_text(
            "heavy_guard() { return 0; }\n")
        verified = self.root / "scripts/verified.sh"
        verified.write_text("#!/bin/sh\nexit 0\n")
        verified.chmod(0o755)
        for tool in ("bend", "bun"):
            path = self.root / "bin" / tool
            path.write_text(FAKE_TOOL)
            path.chmod(0o755)
        self.env = dict(os.environ, PATH=f"{self.root / 'bin'}:{os.environ['PATH']}",
                        TEST_JOBS="2")
        for key in ("TEST_SHARD", "TEST_SHARD_COUNT", "TEST_TIMINGS_OUT",
                    "TEST_FULL", "TEST_FILES",
                    "BUN_JSC_maxPerThreadStackUsage"):
            self.env.pop(key, None)

    def program(self, name: str, body: str = "def main() -> IO(Unit):\n") -> None:
        (self.root / "test" / f"{name}_test.bend").write_text(body)

    def driver(self, name: str, body: str = "# pass\n") -> None:
        (self.root / "test/tools" / f"{name}.mjs").write_text(body)

    def run_suite(self, **env: str) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(["sh", "scripts/test.sh"], cwd=self.root,
                                env=self.env | env, capture_output=True,
                                text=True, timeout=30)
        self.assertNotIn("cannot open scripts/", result.stderr,
                         "fixture setup failed before dispatch")
        return result

    def calls(self) -> list[list[object]]:
        return [json.loads(line) for line in
                (self.root / "calls.jsonl").read_text().splitlines()]

    def test_runs_main_and_javascript_driver_and_skips_negative_control(self) -> None:
        self.program("ordinary")
        self.program("fixture", "def exported() -> String:\n")
        self.driver("fixture")
        self.program("both")
        self.driver("both")
        self.program("cache_identity_before", "# fail\ndef main() -> IO(Unit):\n")
        result = self.run_suite()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.calls()
        args = [row[1] for row in calls]
        self.assertIn(["test/ordinary_test.bend"], args)
        self.assertIn(["test/both_test.bend"], args)
        self.assertNotIn(["test/fixture_test.bend"], args)
        self.assertEqual(sum(row[0] == "bun" for row in calls), 2)
        self.assertFalse(any("cache_identity_before" in str(row) for row in calls))
        self.assertTrue(all(row[2] == "536870912" for row in calls))

    def test_standalone_failures_crashes_and_empty_results_fail_suite(self) -> None:
        for mode in ("fail", "crash", "empty"):
            with self.subTest(mode=mode):
                self.program("ordinary", f"# {mode}\ndef main() -> IO(Unit):\n")
                result = self.run_suite()
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                expected = {"fail": "FAIL regression", "crash": "runtime crashed",
                            "empty": "did not run"}[mode]
                self.assertIn(expected, result.stdout)

    def test_fixture_needs_an_executed_driver(self) -> None:
        self.program("fixture", "def exported() -> String:\n")
        result = self.run_suite()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not run", result.stdout)

    def test_driver_failure_and_emission_failure_fail_suite(self) -> None:
        self.program("fixture", "def exported() -> String:\n")
        for mode in ("fail", "crash", "empty"):
            with self.subTest(mode=mode):
                self.driver("fixture", f"# {mode}\n")
                result = self.run_suite()
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                expected = {"fail": "FAIL regression", "crash": "runtime crashed",
                            "empty": "did not run"}[mode]
                self.assertIn(expected, result.stdout)
        self.driver("fixture")
        self.program("fixture", "# emit-error\ndef exported() -> String:\n")
        result = self.run_suite()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("exited non-zero", result.stdout)

    def test_main_failure_is_not_hidden_by_a_passing_driver(self) -> None:
        self.program("both", "# fail\ndef main() -> IO(Unit):\n")
        self.driver("both")
        result = self.run_suite()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("FAIL regression", result.stdout)

    def test_shards_cover_each_case_once_including_drivers(self) -> None:
        for name in ("a", "b", "c", "d"):
            self.program(name)
        self.program("fixture", "def exported() -> String:\n")
        self.driver("fixture")
        for shard in ("1", "2"):
            result = self.run_suite(TEST_SHARD=shard, TEST_SHARD_COUNT="2")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.calls()
        self.assertEqual(sum(row[0] == "bend" for row in calls), 5)
        self.assertEqual(sum(row[0] == "bun" for row in calls), 1)

    def test_default_runs_only_required_set_and_full_is_explicit(self) -> None:
        self.program('a'); self.program('b')
        (self.root/'scripts/fast-tests.txt').write_text('test/a_test.bend\n')
        self.assertEqual(self.run_suite().returncode,0)
        self.assertEqual([row[1] for row in self.calls()],[['test/a_test.bend']])
        (self.root/'calls.jsonl').unlink()
        self.assertEqual(self.run_suite(TEST_FULL='1').returncode,0)
        self.assertEqual(len(self.calls()),2)

    def test_bad_required_selection_fails_before_executing(self) -> None:
        self.program('a')
        (self.root/'scripts/fast-tests.txt').write_text('test/missing_test.bend\n')
        self.assertEqual(self.run_suite().returncode,2)
        self.assertFalse((self.root/'calls.jsonl').exists())


if __name__ == "__main__":
    unittest.main()
