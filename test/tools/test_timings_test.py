"""Timing import/record controls; no actual Bend, locks, Boat or app execution."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('timing_import', ROOT / 'scripts/import-test-timings.py')
timings = importlib.util.module_from_spec(spec)
spec.loader.exec_module(timings)
REV = 'a' * 40
NAMES = [f'test/{name}_test.bend' for name in 'abcd']


def log(rows, revision=REV):
    return f'2026-11-02T00:00:00Z {revision}\n' + ''.join(
        f'2026-11-02T00:00:01Z {name}: {verdict}\n' for name, verdict in rows)


class TimingImportTest(unittest.TestCase):
    def setUp(self):
        self.groups = [log([(NAMES[0], 'ok (9.0s, 2.0 GB)'),
                            (NAMES[2], 'ok (8.0s, 3.0 GB)')]),
                       log([(NAMES[1], 'ok (1.0s, 1.0 GB)'),
                            (NAMES[3], 'did not run')])]
        self.old = dict.fromkeys(NAMES, 1)

    def test_full_inventory_preserves_failed_unknown_and_only_imports_durations(self):
        weights, report = timings.combine(self.groups, REV, NAMES, self.old)
        self.assertEqual(weights, dict(zip(NAMES[:3], [9, 1, 8])))
        self.assertIn(NAMES[3], report['untimed_or_failed'])
        self.assertEqual(report['inventory_count'], 4)
        self.assertEqual(report['peak_gib_max'], 3)
        groups = timings.shard.partition(NAMES, weights, 2)
        self.assertCountEqual([name for group in groups for name in group], NAMES)

    def test_missing_or_foreign_program_rejected(self):
        for expected in [NAMES[:3], [*NAMES, 'test/new_test.bend']]:
            with self.subTest(expected=expected), self.assertRaisesRegex(ValueError, 'inventory'):
                timings.combine(self.groups, REV, expected, self.old)

    def test_wrong_shard_order_rejected(self):
        with self.assertRaisesRegex(ValueError, 'assignment'):
            timings.combine(self.groups[::-1], REV, NAMES, self.old)

    def test_duplicate_and_stale_revision_rejected(self):
        with self.assertRaisesRegex(ValueError, 'multiple shards'):
            timings.combine([self.groups[0], self.groups[0]], REV, NAMES, self.old)
        with self.assertRaisesRegex(ValueError, 'duplicate verdict'):
            timings.parse_log(self.groups[0] + self.groups[0], REV)
        with self.assertRaisesRegex(ValueError, 'revision'):
            timings.parse_log(self.groups[0], 'b' * 40)


class TimingRecorderTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'scripts').mkdir()
        shutil.copyfile(ROOT / 'scripts/record-test-timings.sh',
                        self.root / 'scripts/record-test-timings.sh')
        (self.root / 'scripts/test-timings.txt').write_text('old profile\n')
        fake = self.root / 'scripts/test.sh'
        fake.write_text('''#!/usr/bin/env python3
import json, os
from pathlib import Path
names=['TEST_FULL','TEST_JOBS','TEST_FILES','TEST_SHARD','TEST_SHARD_COUNT']
Path('called.json').write_text(json.dumps({k:os.environ.get(k) for k in names}))
Path(os.environ['TEST_TIMINGS_OUT']).write_text('2.00 test/a_test.bend\\n')
raise SystemExit(int(os.environ.get('FAKE_EXIT','0')))
''')
        fake.chmod(0o755)

    def run_recorder(self, rc=0):
        return subprocess.run(['sh', 'scripts/record-test-timings.sh'], cwd=self.root,
                              env={**os.environ, 'TEST_FILES': 'test/narrow_test.bend',
                                   'TEST_FULL': '0', 'TEST_JOBS': '6', 'TEST_SHARD': '3',
                                   'TEST_SHARD_COUNT': '4', 'FAKE_EXIT': str(rc)},
                              text=True, capture_output=True, timeout=5)

    def test_forces_complete_serial_selection_and_removes_inherited_shards(self):
        result = self.run_recorder()
        self.assertEqual(result.returncode, 0, result.stderr)
        called = json.loads((self.root / 'called.json').read_text())
        self.assertEqual(called, dict(TEST_FULL='1', TEST_JOBS='1', TEST_FILES=None,
                                      TEST_SHARD=None, TEST_SHARD_COUNT=None))
        self.assertIn('2.00 test/a_test.bend', (self.root / 'scripts/test-timings.txt').read_text())
        self.assertEqual(list((self.root / 'scripts').glob('test-timings.*')), [self.root / 'scripts/test-timings.txt'])

    def test_failure_cannot_replace_existing_profile(self):
        result = self.run_recorder(1)
        self.assertEqual(result.returncode, 1)
        self.assertEqual((self.root / 'scripts/test-timings.txt').read_text(), 'old profile\n')
        self.assertEqual(list((self.root / 'scripts').glob('test-timings.*')), [self.root / 'scripts/test-timings.txt'])


if __name__ == '__main__':
    unittest.main()
