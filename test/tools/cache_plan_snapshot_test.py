"""A cached plan is consumed only against its complete current snapshot."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'snapshot_fixture', Path(__file__).with_name('content_cache_test.py'))
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
cc = fixture.cc
ROOT = Path(__file__).resolve().parents[2]


class Snapshots(unittest.TestCase):
    setUp = fixture.Base.setUp
    write = fixture.Base.write
    mark_all_passed = fixture.Base.mark_all_passed
    fake_bend = fixture.RunCheck.fake_bend
    run_check = fixture.RunCheck.run_check

    def plan_file(self):
        with contextlib.redirect_stdout(io.StringIO()):
            return fixture.RunCheck.plan_file(self)

    def warm_plan(self):
        self.mark_all_passed()  # test-only local authenticated pass fixtures
        return self.plan_file()

    def no_bend(self):
        marker = self.root.parent/'BEND-STARTED'
        self.fake_bend('touch %s; exit 97\n' % shlex.quote(str(marker)))
        return marker

    def test_unchanged_all_hit_starts_no_bend_or_heavy_guard(self):
        marker = self.no_bend()
        plan = self.warm_plan()
        with patch.object(cc, '_module', side_effect=AssertionError('no heavy guard needed')):
            self.assertEqual(cc.run_check(self.root, plan), 0)
        self.assertFalse(marker.exists())

    def test_transitive_edit_after_warm_plan_refuses_without_bend(self):
        marker = self.no_bend()
        plan = self.warm_plan()
        self.write('src/leaf.bend', 'invalid changed theorem\n')
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)
        self.assertFalse(marker.exists())

    def test_new_or_deleted_module_refuses_warm_plan(self):
        plan = self.warm_plan()
        self.write('src/new.bend', 'import Base\ndef new() -> U32:\n  3\n')
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)
        (self.root/'src/new.bend').unlink()
        (self.root/'src/other.bend').unlink()
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)

    def test_deleted_or_corrupt_hit_entry_refuses(self):
        plan = self.warm_plan()
        key = cc.check_units(self.root)['src/other.bend'][0]
        path = cc.entry_path(self.cache, key)
        original = path.read_bytes()
        path.unlink()
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)
        path.write_bytes(original)
        self.assertEqual(cc.run_check(self.root, plan), 0)
        path.write_text('{}')
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)

    def test_tool_change_refuses_all_hit(self):
        plan = self.warm_plan()
        self.tool = {**self.tool, 'env': {'BEND_LIB': 'changed'}}
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)

    def test_partial_old_duplicate_or_unknown_plans_refuse(self):
        plan = self.warm_plan()
        valid = json.loads(plan.read_text())
        old = {k: v for k, v in valid.items() if k != 'modules'}
        bad_keys = {**valid, 'keys': {}}
        duplicate = {**valid, 'miss': ['src/other.bend']*2}
        unknown = {**valid, 'miss': ['src/not-in-the-plan.bend']}
        empty = {'schema': 1, 'modules': [], 'keys': {}, 'miss': []}
        for bad in (old, bad_keys, duplicate, unknown, empty):
            with self.subTest(plan=bad):
                plan.write_text(json.dumps(bad))
                with self.assertRaises(cc.CacheError):
                    cc.run_check(self.root, plan)

    def test_omitted_uncached_module_is_no_hit(self):
        plan = self.plan_file()
        planned = json.loads(plan.read_text())
        planned['miss'] = []  # pretend that a cold plan has no work
        plan.write_text(json.dumps(planned))
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)

    def test_uncacheable_unit_cannot_be_declared_a_hit(self):
        self.write('src/foreign.bend', 'import Base\nimport ./missing_header.c\n')
        self.write('src/missing_header.c', '#include "unavailable.h"\n')
        plan = self.plan_file()
        planned = json.loads(plan.read_text())
        planned['miss'] = []
        plan.write_text(json.dumps(planned))
        with self.assertRaises(cc.CacheError):
            cc.run_check(self.root, plan)

    def mixed_plan(self):
        self.mark_all_passed()
        self.write('src/other.bend', 'import Base\ndef other() -> U32:\n  3\n')
        plan = self.plan_file()
        self.assertEqual(json.loads(plan.read_text())['miss'], ['src/other.bend'])
        return plan

    def test_cached_hit_edit_while_miss_runs_refuses_and_records_nothing(self):
        plan = self.mixed_plan()
        self.fake_bend('printf "\\ninvalid change\\n" >> %s\necho "All terms check."\n' %
                       shlex.quote(str(self.root/'src/leaf.bend')))
        with self.assertRaises(cc.CacheError):
            self.run_check(plan)
        key = cc.check_units(self.root)['src/other.bend'][0]
        self.assertFalse(cc.lookup(key))

    def test_hit_entry_removed_while_miss_runs_refuses(self):
        plan = self.mixed_plan()
        key = cc.check_units(self.root)['src/leaf.bend'][0]
        self.fake_bend('rm -f %s\necho "All terms check."\n' %
                       shlex.quote(str(cc.entry_path(self.cache, key))))
        with self.assertRaises(cc.CacheError):
            self.run_check(plan)

    def test_new_module_while_miss_runs_refuses(self):
        plan = self.mixed_plan()
        self.fake_bend('printf "import Base\\ndef new() -> U32:\\n  0\\n" > %s\necho "All terms check."\n' %
                       shlex.quote(str(self.root/'src/new.bend')))
        with self.assertRaises(cc.CacheError):
            self.run_check(plan)

    def test_force_all_plan_ignores_cache_but_checks_every_module(self):
        self.mark_all_passed()
        plan = self.plan_file()
        planned = json.loads(plan.read_text())
        planned['miss'] = planned['modules']
        plan.write_text(json.dumps(planned))
        marker = self.root.parent/'BEND-STARTED'
        self.fake_bend('touch %s; echo "All terms check."\n' % shlex.quote(str(marker)))
        self.assertEqual(self.run_check(plan), 0)
        self.assertTrue(marker.exists())


class ShellRoute(unittest.TestCase):
    def test_all_hit_shell_propagates_validator_failure_without_guard(self):
        # Isolate the shell's control flow. Real key/MAC behavior is exercised
        # above; stand-ins here cannot invoke Bend or acquire the shared slot.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'scripts').mkdir()
            (root/'bin').mkdir()
            shutil.copyfile(ROOT/'scripts/cached-check.sh', root/'scripts/cached-check.sh')
            (root/'scripts/bendok.sh').write_text('')
            (root/'scripts/heavy-guard.sh').write_text('heavy_guard() { echo BAD-GUARD; return 99; }\n')
            program = root/'bin/python3'
            program.write_text('''#!/bin/sh
case "$1:$2" in
  scripts/laws-shape.py:*) exit 0 ;;
  scripts/content-cache.py:plan-check)
    printf '{"hit":["a.bend"],"miss":[]}\\n'
    exit 0 ;;
  scripts/content-cache.py:run-check)
    echo VALIDATOR-REFUSED >&2
    exit 42 ;;
  -c:*)
    case "$2" in *'len('*) echo 1 ;; esac
    exit 0 ;;
esac
exit 98
''')
            program.chmod(0o755)
            import os
            env = {**os.environ, 'PATH': str(root/'bin')+os.pathsep+os.environ['PATH']}
            result = subprocess.run(['sh', 'scripts/cached-check.sh'], cwd=root, env=env,
                                    text=True, capture_output=True)
            self.assertEqual(result.returncode, 42, result)
            self.assertIn('VALIDATOR-REFUSED', result.stderr)
            self.assertNotIn('all 1 modules cached', result.stdout)
            self.assertNotIn('BAD-GUARD', result.stdout)


if __name__ == '__main__':
    unittest.main()
