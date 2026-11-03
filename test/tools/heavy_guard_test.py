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

    def test_compile_only_profile_does_not_lower_other_floors_or_reserve(self):
        self.assertEqual(guard.FLOORS['build'], 16 * guard.GIB)
        self.assertEqual(guard.FLOORS['compile-js'], 3 * guard.GIB)
        with patch.object(guard, 'available', return_value=3 * guard.GIB), patch.object(guard, 'host_available', return_value=11 * guard.GIB):
            self.assertTrue(guard.admit('compile-js'))
            self.assertFalse(guard.admit('build'))
        with patch.object(guard, 'available', return_value=3 * guard.GIB), patch.object(guard, 'host_available', return_value=11 * guard.GIB - 1):
            self.assertFalse(guard.admit('compile-js'))

    def test_retained_bytes_never_use_source_only_automatic_transport(self):
        with patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.offload(['scripts/build-js.sh', 'a.bend', 'dist/a'], {'BACKPLANE_PREEMITTED_JS': '/retained/server.js'}), 75)
            run.assert_not_called()

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

    def cgroup(self, limit, swap):
        d = Path(self.temp.name) / ('cg-%s-%s' % (limit, swap))
        d.mkdir(exist_ok=True)
        if limit is not None:
            (d / 'memory.max').write_text(limit + '\n')
        if swap is not None:
            (d / 'memory.swap.max').write_text(swap + '\n')
        return d

    def test_shard_profile_keeps_existing_floors_and_has_own_lower_floor(self):
        self.assertEqual(guard.FLOORS['proof'], 19 * guard.GIB)
        self.assertEqual(guard.FLOORS['check'], 14 * guard.GIB)
        self.assertEqual(guard.HOST_RESERVE, 8 * guard.GIB)
        self.assertEqual(guard.FLOORS['proof-shard'], 3 * guard.GIB)
        self.assertEqual(guard.FLOORS['compile-shard'], 3 * guard.GIB)
        self.assertEqual(guard.SHARD_CAP, 3_700_000_000)
        self.assertLess(guard.SHARD_CAP + guard.CONTROLLER_CAP, 4 * guard.GIB)

    def test_shard_cgroup_must_be_finite_capped_and_noswap(self):
        cap = str(guard.SHARD_CAP)
        self.assertTrue(guard.verify_shard_cgroup(self.cgroup(cap, '0')))
        for limit, swap in [('max', '0'), (str(guard.SHARD_CAP + 1), '0'), (cap, 'max'), (cap, '1'),
                            ('0', '0'), (cap, None), (None, '0'), ('-5', '0')]:
            self.assertFalse(guard.verify_shard_cgroup(self.cgroup(limit, swap)), (limit, swap))

    def test_shard_profile_refuses_unverifiable_cgroup(self):
        argv = ['proof-shard', '--', sys.executable, str(ROOT / 'scripts/proof-shards.py'), 'run', '--out', 'd', '--shard', '000', '--expect', 'x', '--result', 'r']
        with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', side_effect=ValueError('none')), patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.main(argv), 75)
            run.assert_not_called()
        with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=self.cgroup('max', '0')), patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.main(argv), 75)
            run.assert_not_called()

    def test_shard_profile_cannot_run_full_proof_or_other_commands(self):
        good = self.cgroup(str(guard.SHARD_CAP), '0')
        shard = [sys.executable, str(ROOT / 'scripts/proof-shards.py'), 'run']
        full = [['bend', 'PROOF.bend'], ['scripts/check.sh', 'proof'], ['sh', '-c', 'bend PROOF.bend'],
                [sys.executable, str(ROOT / 'scripts/proof-shards.py'), 'generate', '--out', 'x'],
                [sys.executable, str(ROOT / 'scripts/proof-shards.py'), 'join', '--out', 'x'],
                [sys.executable, '/tmp/proof-shards.py', 'run', '--shard', '0', '--expect', 'x', '--result', 'r'],
                [*shard, '--result', 'r'], [*shard, '--shard', '0', '--shard', '1', '--expect', 'x', '--result', 'r']]
        with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=good), patch.object(guard.os, 'execvpe') as run:
            for command in full:
                self.assertEqual(guard.main(['proof-shard', '--', *command]), 2, command)
            self.assertEqual(guard.main(['--held', 'proof-shard']), 2)
            run.assert_not_called()

    def test_shard_profile_runs_exact_worker_when_capped_and_never_offloads(self):
        good = self.cgroup(str(guard.SHARD_CAP), '0')
        argv = ['proof-shard', '--', sys.executable, str(ROOT / 'scripts/proof-shards.py'), 'run', '--out', 'd', '--shard', '000', '--expect', 'x', '--result', 'r']
        with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=good), patch.object(guard.os, 'execvpe') as run:
            guard.main(argv)
            self.assertEqual(run.call_args.args[0], str(Path(sys.executable).resolve()))
        other = Path(self.temp.name) / 'second.lock'
        with open(other, 'w') as held, patch.object(guard, 'lock_path', return_value=other):
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=good), patch.object(guard.os, 'execvpe') as run, patch.object(guard, 'offload') as off:
                self.assertEqual(guard.main(argv), 75)
                run.assert_not_called()
                off.assert_not_called()

    def test_shard_floor_is_admission_only_low_memory_refuses(self):
        with patch.object(guard, 'available', return_value=guard.FLOORS['proof-shard'] - 1):
            self.assertFalse(guard.admit('proof-shard'))

    def wrapped(self, inner, **over):
        opts = {'--id': 'x', '--out': 'build/x', '--cap-bytes': str(guard.SHARD_CAP), '--timeout': '900'}
        opts.update(over)
        flat = [t for k, v in opts.items() if v is not None for t in (k, v)]
        return [sys.executable, str(ROOT / 'scripts/lowmem-build.py'), 'worker', *flat, '--', *inner]

    SHARD = ['python3', str(ROOT / 'scripts/proof-shards.py'), 'run', '--out', 'd', '--shard', '1', '--expect', 'h', '--result', 'r']

    def test_compile_shard_allows_only_the_three_exact_compiles(self):
        for command in guard.COMPILE_COMMANDS:
            self.assertIsNotNone(guard.worker_contract('compile-shard', self.wrapped(list(command))), command)
        for bad in [['bend', 'PROOF.bend'], ['bend', 'src/server/main.bend', '-o', '/tmp/x'], ['sh', '-c', 'bend PROOF.bend'],
                    ['bend', 'src/server/main.bend', '-o', '{out}/hub.js', '--extra'], ['scripts/check.sh', 'proof'],
                    ['bend', 'src/web/index.html', '-o', '{out}/hub.js']]:
            self.assertIsNone(guard.worker_contract('compile-shard', self.wrapped(bad)), bad)
        # compile-shard cannot run proof shards, nor proof-shard compiles
        self.assertIsNone(guard.worker_contract('compile-shard', self.wrapped(self.SHARD)))
        self.assertIsNone(guard.worker_contract('proof-shard', self.wrapped(list(guard.COMPILE_COMMANDS[0]))))

    def test_wrapper_options_are_closed_and_cap_bounded(self):
        hub = list(guard.COMPILE_COMMANDS[0])
        for over in [{'--cap-bytes': str(guard.SHARD_CAP + 1)}, {'--cap-bytes': '0'}, {'--cap-bytes': 'big'}, {'--cap-bytes': None}, {'--timeout': None},
                     {'--no-scope': ''}, {'--unknown': '1'}]:
            self.assertIsNone(guard.worker_contract('compile-shard', self.wrapped(hub, **over)), over)
        dup = self.wrapped(hub)
        dup[3:3] = ['--id', 'again']
        self.assertIsNone(guard.worker_contract('compile-shard', dup))
        self.assertIsNone(guard.worker_contract('compile-shard', self.wrapped(hub)[:2] + ['worker']))

    def test_proof_shard_wrapped_requires_the_exact_shard_run(self):
        self.assertIsNotNone(guard.worker_contract('proof-shard', self.wrapped(self.SHARD)))
        self.assertIsNone(guard.worker_contract('proof-shard', self.wrapped(['sh', '-c', 'bend PROOF.bend'])))
        self.assertIsNone(guard.worker_contract('proof-shard', self.wrapped(self.SHARD[:3])))

    def test_interpreter_must_be_the_guards_own(self):
        other = Path(self.temp.name) / 'python3'
        other.write_text('#!/bin/sh\nexit 0\n')
        other.chmod(0o755)
        for exe in [str(other), '/bin/sh', 'python3.99']:
            self.assertIsNone(guard.worker_contract('proof-shard', [exe, *self.SHARD[1:]]), exe)
            self.assertIsNone(guard.worker_contract('compile-shard', [exe, *self.wrapped(list(guard.COMPILE_COMMANDS[0]))[1:]]), exe)
        pinned = guard.worker_contract('proof-shard', self.SHARD)
        self.assertEqual(pinned[0], str(Path(sys.executable).resolve()))

    def test_compile_shard_fails_closed_and_never_offloads(self):
        command = self.wrapped(list(guard.COMPILE_COMMANDS[1]))
        argv = ['compile-shard', '--', *command]
        for limit, swap in [('max', '0'), (str(guard.SHARD_CAP + 1), '0'), (str(guard.SHARD_CAP), 'max'), (str(guard.SHARD_CAP), '1')]:
            with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=self.cgroup(limit, swap)), patch.object(guard.os, 'execvpe') as run, patch.object(guard, 'offload') as off:
                self.assertEqual(guard.main(argv), 75, (limit, swap))
                run.assert_not_called()
                off.assert_not_called()
        good = self.cgroup(str(guard.SHARD_CAP), '0')
        with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=good), patch.object(guard.os, 'execvpe') as run:
            guard.main(argv)
            self.assertEqual(run.call_args.args[0], str(Path(sys.executable).resolve()))
        with patch.object(guard, 'available', return_value=100 * guard.GIB), patch.object(guard, 'own_cgroup', return_value=good), patch.object(guard.os, 'execvpe') as run:
            self.assertEqual(guard.main(['compile-shard', '--held']), 2)
            self.assertEqual(guard.main(['--held', 'compile-shard']), 2)
            self.assertEqual(guard.main(['compile-shard', '--', 'bend', 'PROOF.bend']), 2)
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
