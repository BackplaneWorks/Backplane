import fcntl
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('heavy_guard', ROOT / 'scripts/heavy-guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
real_lock_path = guard.lock_path


class HeavyGuardTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.env = patch.dict(os.environ, {'BACKPLANE_LOCK_DIR': self.temp.name}, clear=False)
        self.env.start()
        self.addCleanup(self.env.stop)
        os.environ.pop(guard.FD_VAR, None)
        os.environ.pop(guard.BOAT_VAR, None)
        os.environ.pop('BACKPLANE_HEAVY_OFFLOADED', None)
        lock = patch.object(guard, 'lock_path', return_value=Path(self.temp.name) / 'bp-bend.lock')
        lock.start()
        self.addCleanup(lock.stop)
        host = patch.object(guard, 'host_available', return_value=100 * guard.GIB)
        host.start()
        self.addCleanup(host.stop)

    def test_expected_peak_and_host_reserve_are_both_required(self):
        expected = guard.FLOORS['build']
        with patch.object(guard, 'available', return_value=expected), patch.object(guard, 'host_available', return_value=expected + guard.HOST_RESERVE - 1):
            self.assertFalse(guard.admit('build'))
        with patch.object(guard, 'available', return_value=expected), patch.object(guard, 'host_available', return_value=expected + guard.HOST_RESERVE):
            self.assertTrue(guard.admit('build'))

    def test_cgroup_headroom_cannot_be_replaced_by_host_memory(self):
        with patch.object(guard, 'available', return_value=guard.FLOORS['proof'] - 1):
            self.assertFalse(guard.admit('proof'))

    def test_boat_uses_sandbox_headroom_not_local_reserve(self):
        expected = guard.FLOORS['web']
        with patch.dict(os.environ, {guard.BOAT_VAR: '1', 'BACKPLANE_HEAVY_OFFLOADED': '1'}), patch.object(guard, 'host_available', return_value=expected), patch.object(guard, 'available', return_value=expected):
            self.assertTrue(guard.admit('web'))
        with patch.dict(os.environ, {guard.BOAT_VAR: '1', 'BACKPLANE_HEAVY_OFFLOADED': '1'}), patch.object(guard, 'available', return_value=expected - 1):
            self.assertFalse(guard.admit('web'))

    def test_one_boat_marker_does_not_skip_host_reserve(self):
        expected = guard.FLOORS['web']
        with patch.dict(os.environ, {guard.BOAT_VAR: '1'}), patch.object(guard, 'host_available', return_value=expected), patch.object(guard, 'available', return_value=expected):
            self.assertFalse(guard.admit('web'))

    def test_named_lock_directory_does_not_change_shared_slot(self):
        with patch.dict(os.environ, {'BACKPLANE_LOCK_DIR': '/tmp/other-name'}):
            self.assertEqual(real_lock_path(), Path('/tmp/bp-bend.lock'))

    def test_different_profiles_contend_on_same_native_slot(self):
        with open(Path(self.temp.name) / 'bp-bend.lock', 'w') as held:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard.os, 'execvpe') as run:
                for name, profile in [('bend', 'proof'), ('backplane-build', 'build'), ('browser', 'browser')]:
                    with patch.dict(os.environ, {'BACKPLANE_LOCK_NAME': name}):
                        self.assertEqual(guard.main([profile, '--', 'anything']), 75)
                run.assert_not_called()

    def test_low_memory_refuses_without_starting(self):
        with patch.object(guard, 'available', return_value=guard.FLOORS['test'] - 1), patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.main(['test', '--', 'anything']), 75)
            run.assert_not_called()

    def test_unreadable_memory_refuses(self):
        with patch.object(guard, 'available', side_effect=OSError('unavailable')), patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.main(['check', '--', 'anything']), 75)
            run.assert_not_called()

    def test_existing_native_flock_refuses(self):
        with open(Path(self.temp.name) / 'bp-bend.lock', 'w') as held:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard.os, 'execvpe') as run:
                self.assertEqual(guard.main(['check', '--', 'anything']), 75)
                run.assert_not_called()

    def test_nested_process_inherits_real_lock(self):
        script = """import importlib.util,sys
s=importlib.util.spec_from_file_location('g',sys.argv[1]);g=importlib.util.module_from_spec(s);s.loader.exec_module(g);g.available=lambda:100*g.GIB;g.host_available=lambda:100*g.GIB;sys.exit(g.main(sys.argv[2:]))"""
        wrapper = [sys.executable, '-c', script, str(ROOT / 'scripts/heavy-guard.py')]
        child = [*wrapper, 'check', '--', sys.executable, '-c', 'print("nested-ok")']
        p = subprocess.run([*wrapper, 'check', '--', *child], text=True, capture_output=True, timeout=5)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertEqual(p.stdout.strip(), 'nested-ok')

    def test_eligible_clean_command_offloads_without_local_execution(self):
        fake = subprocess.CompletedProcess([], 0, '', '')
        with patch.object(guard.subprocess, 'run', return_value=fake), patch.object(guard.os, 'execvpe', side_effect=OSError('stop')) as run, patch.object(guard.os, 'chdir'):
            self.assertEqual(guard.offload(['scripts/test.sh'], {}), 75)
            argv = run.call_args.args[1]
            self.assertIn('--no-reuse', argv)
            self.assertIn('scripts/heavy-boat.sh', argv)
            self.assertIn('scripts/test.sh', argv)

    def test_dirty_source_never_transfers(self):
        fake = subprocess.CompletedProcess([], 0, ' M private-file\n', '')
        with patch.object(guard.subprocess, 'run', return_value=fake), patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.offload(['scripts/test.sh'], {}), 75)
            run.assert_not_called()

    def test_remote_failure_does_not_offload_recursively(self):
        with patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.offload(['scripts/test.sh'], {'BACKPLANE_HEAVY_OFFLOADED': '1'}), 75)
            run.assert_not_called()

    def test_false_descriptor_does_not_bypass_lock(self):
        os.environ[guard.FD_VAR] = '9999'
        self.assertIsNone(guard.inherited_lock(Path(self.temp.name) / 'bp-bend.lock'))


if __name__ == '__main__':
    unittest.main()
