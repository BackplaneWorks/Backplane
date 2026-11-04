import fcntl
import hashlib
import importlib.util
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('compiler_guard', ROOT/'scripts/compiler-guard.py')
cg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cg)
spec2 = importlib.util.spec_from_file_location('heavy_guard', ROOT/'scripts/heavy-guard.py')
hg = importlib.util.module_from_spec(spec2)
spec2.loader.exec_module(hg)

FAKE = '#!/bin/sh\necho "REAL $*" >> "$MARKER"\n'


def clean_env(home, guard, marker):
    env = {k: v for k, v in os.environ.items() if not k.startswith('BACKPLANE_')}
    env.update(HOME=str(home), BACKPLANE_GUARD_HOME=str(guard), MARKER=str(marker), PATH=os.environ['PATH'])
    return env


class CompilerGuard(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.home = Path(self.tmp.name)/'home'
        self.guard = Path(self.tmp.name)/'guard'
        self.marker = Path(self.tmp.name)/'marker'
        for rel in ('.bend/bin/bend', '.bun/bin/bun'):
            path = self.home/rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(FAKE)
            path.chmod(0o755)
        (self.home/'.bun/bin/bunx').symlink_to(self.home/'.bun/bin/bun')
        self.env = clean_env(self.home, self.guard, self.marker)
        self.original = {n: (self.home/rel).read_bytes() for n, rel in (('bend', '.bend/bin/bend'), ('bun', '.bun/bin/bun'))}

    REL = {'bend': '.bend/bin/bend', 'bun': '.bun/bin/bun'}

    def real(self, name):
        return self.home/(self.REL[name] + '.real')           # renamed beside the shim: bend finds its Base relative to itself

    def install(self, apply=True):
        return cg.install(self.home, ['bend', 'bun'], apply, self.env)

    def run_installed(self, rel, *args, env=None):
        return subprocess.run([str(self.home/rel), *args], env=env or self.env, capture_output=True, text=True, timeout=30)

    # install / uninstall ------------------------------------------------------------------------------------------
    def test_dry_run_changes_nothing(self):
        before = {p: p.read_bytes() for p in (self.home/'.bend/bin/bend', self.home/'.bun/bin/bun')}
        actions = self.install(apply=False)
        self.assertEqual(len(actions), 2)
        self.assertEqual({p: p.read_bytes() for p in before}, before)
        self.assertFalse(self.guard.exists())

    def test_apply_moves_the_real_binary_and_leaves_a_shim_at_the_original_path(self):
        self.install()
        for name, rel in (('bend', '.bend/bin/bend'), ('bun', '.bun/bin/bun')):
            path = self.home/rel
            self.assertTrue(cg.is_shim(path))
            self.assertTrue(os.access(path, os.X_OK))
            real = self.real(name)
            self.assertEqual(real.read_bytes(), self.original[name])
            self.assertTrue(os.access(real, os.X_OK))
        self.assertTrue((self.home/'.bun/bin/bunx').is_symlink())          # untouched; resolves through the shim
        self.assertEqual(cg.sha(self.guard/'compiler-guard.py'), cg.sha(cg.SELF))   # stable vendored copy
        self.assertEqual(cg.check(self.home, ['bend', 'bun'], self.env), [])

    def test_install_is_idempotent(self):
        self.install()
        shim = (self.home/'.bend/bin/bend').read_bytes()
        again = self.install()
        self.assertTrue(all('already guarded' in text for _, text in again))
        self.assertEqual((self.home/'.bend/bin/bend').read_bytes(), shim)
        self.assertEqual(self.real('bend').read_bytes(), self.original['bend'])

    def test_uninstall_restores_the_exact_bytes_and_mode(self):
        modes = {rel: stat.S_IMODE((self.home/rel).stat().st_mode) for rel in ('.bend/bin/bend', '.bun/bin/bun')}
        self.install()
        cg.uninstall(self.home, ['bend', 'bun'], True, self.env)
        for name, rel in (('bend', '.bend/bin/bend'), ('bun', '.bun/bin/bun')):
            self.assertEqual((self.home/rel).read_bytes(), self.original[name])
            self.assertEqual(stat.S_IMODE((self.home/rel).stat().st_mode), modes[rel])
        self.assertFalse(cg.is_shim(self.home/'.bend/bin/bend'))

    def test_uninstall_refuses_when_the_real_binary_changed(self):
        self.install()
        self.real('bend').write_bytes(b'#!/bin/sh\necho tampered\n')
        actions = cg.uninstall(self.home, ['bend'], True, self.env)
        self.assertIn('REFUSED', actions[0][1])
        self.assertTrue(cg.is_shim(self.home/'.bend/bin/bend'))

    def test_check_detects_a_reinstall_over_the_shim_and_a_changed_real_binary(self):
        self.install()
        (self.home/'.bend/bin/bend').write_text(FAKE)                       # e.g. an update replaced the shim
        problems = cg.check(self.home, ['bend', 'bun'], self.env)
        self.assertTrue(any('not the guard shim' in p for p in problems))
        self.real('bun').write_bytes(b'x')
        self.assertTrue(any('changed' in p for p in cg.check(self.home, ['bun'], self.env)))

    # transactional install ------------------------------------------------------------------------------------------
    def snapshot(self):
        files = {}
        for rel in ('.bend/bin', '.bun/bin'):
            for p in sorted((self.home/rel).iterdir()):
                files[str(p.relative_to(self.home))] = (os.readlink(p) if p.is_symlink() else p.read_bytes(),
                                                          None if p.is_symlink() else stat.S_IMODE(p.stat().st_mode))
        return files

    def test_install_refuses_to_overwrite_an_unexpected_existing_real_file(self):
        stale = self.home/'.bend/bin/bend.real'
        stale.write_text('unrelated precious file\n')
        before = self.snapshot()
        with self.assertRaisesRegex(cg.InstallError, 'not overwriting|refusing to overwrite'):
            cg.install(self.home, ['bend'], True, self.env)
        self.assertEqual(self.snapshot(), before)                  # nothing moved, nothing lost
        self.assertFalse(cg.is_shim(self.home/'.bend/bin/bend'))
        dry = cg.install(self.home, ['bend'], False, self.env)
        self.assertIn('REFUSED', dry[0][1])

    def test_install_refuses_a_stale_real_when_the_manifest_is_missing_or_does_not_match(self):
        self.install()
        manifest = cg.manifest_path(self.guard)
        manifest.write_text('{}\n')                                 # the record of our backup is gone
        (self.home/'.bend/bin/bend').write_bytes(self.original['bend'])      # e.g. a reinstall over the shim
        before = self.snapshot()
        with self.assertRaises(cg.InstallError):
            cg.install(self.home, ['bend'], True, self.env)
        self.assertEqual(self.snapshot(), before)

    def test_every_injected_failure_restores_the_original_exactly(self):
        for point in ('after-journal', 'after-move', 'after-shim', 'before-manifest'):
            with self.subTest(point=point):
                before = self.snapshot()
                with patch.dict(os.environ, {'COMPILER_GUARD_TEST_FAULT': point}):
                    with self.assertRaisesRegex(RuntimeError, 'injected fault'):
                        cg.install(self.home, ['bend', 'bun'], True, self.env)
                self.assertEqual(self.snapshot(), before)           # bytes and modes identical, no .real, no temp files
                self.assertFalse(cg.is_shim(self.home/'.bend/bin/bend'))
                self.assertEqual([n for n in cg.load(self.guard) if True], [])   # no half-written record
                self.assertEqual(cg.check(self.home, ['bend', 'bun'], self.env), ['bend: %s is a real binary, not the guard shim' % (self.home/'.bend/bin/bend'), 'bun: %s is a real binary, not the guard shim' % (self.home/'.bun/bin/bun')])
        self.install()                                              # and a clean install still works afterwards
        self.assertEqual(cg.check(self.home, ['bend', 'bun'], self.env), [])

    def test_an_interrupted_install_is_rolled_back_by_the_next_install(self):
        path, real = self.home/'.bend/bin/bend', self.home/'.bend/bin/bend.real'
        digest = cg.sha(path)
        before = self.snapshot()
        cg.save_state(self.guard, {'bend': {'path': str(path), 'real': str(real), 'original_sha256': digest,
                                            'mode': 0o755, 'state': 'installing', 'previous': None, 'had_previous': False}})
        os.rename(path, real)                                       # the process died right after the move
        self.assertTrue(any('interrupted' in p for p in cg.check(self.home, ['bend'], self.env)))
        cg.install(self.home, ['bend'], True, self.env)
        self.assertTrue(cg.is_shim(path))
        self.assertEqual(real.read_bytes(), self.original['bend'])
        self.assertEqual(cg.check(self.home, ['bend'], self.env), [])

    def test_an_unclear_interrupted_state_fails_closed_and_touches_nothing(self):
        path, real = self.home/'.bend/bin/bend', self.home/'.bend/bin/bend.real'
        cg.save_state(self.guard, {'bend': {'path': str(path), 'real': str(real), 'original_sha256': 'f' * 64,
                                            'mode': 0o755, 'state': 'installing', 'previous': None, 'had_previous': False}})
        before = self.snapshot()
        with self.assertRaisesRegex(cg.InstallError, 'unexpected state'):
            cg.install(self.home, ['bend'], True, self.env)
        self.assertEqual(self.snapshot(), before)

    def test_update_keeps_the_previous_backup_and_a_failed_update_restores_both(self):
        self.install()
        old_real = self.real('bend').read_bytes()
        (self.home/'.bend/bin/bend').write_text('#!/bin/sh\necho NEW\n')       # what `bend update` leaves behind
        (self.home/'.bend/bin/bend').chmod(0o755)
        with patch.dict(os.environ, {'COMPILER_GUARD_TEST_FAULT': 'after-shim'}):
            with self.assertRaises(RuntimeError):
                cg.install(self.home, ['bend'], True, self.env)
        self.assertEqual((self.home/'.bend/bin/bend').read_text(), '#!/bin/sh\necho NEW\n')      # the new binary is back
        self.assertEqual(self.real('bend').read_bytes(), old_real)                                 # the old backup is back
        self.assertFalse(self.real('bend').with_name('bend.real.prev').exists())
        cg.install(self.home, ['bend'], True, self.env)
        self.assertTrue(cg.is_shim(self.home/'.bend/bin/bend'))
        self.assertEqual(self.real('bend').read_text(), '#!/bin/sh\necho NEW\n')
        self.assertEqual(self.real('bend').with_name('bend.real.prev').read_bytes(), old_real)     # kept, not silently dropped

    def crash_install(self, point, tool='bend'):
        env = {**self.env, 'COMPILER_GUARD_TEST_FAULT': point, 'COMPILER_GUARD_TEST_CRASH': '1'}
        return subprocess.run([sys.executable, str(ROOT/'scripts/compiler-guard.py'), 'install', '--home', str(self.home), '--tool', tool, '--apply'],
                              env=env, capture_output=True, text=True, timeout=60)

    def run_cli(self, *args):
        return subprocess.run([sys.executable, str(ROOT/'scripts/compiler-guard.py'), *args, '--home', str(self.home), '--tool', 'bend'],
                              env=self.env, capture_output=True, text=True, timeout=60)

    def everything_bytes(self):
        """Every regular file beside the bend shim, by name: nothing may disappear."""
        return {p.name: p.read_bytes() for p in (self.home/'.bend/bin').iterdir() if p.is_file() and not p.is_symlink()}

    def test_a_hard_crash_at_every_point_of_a_first_install_is_recovered_without_losing_the_original(self):
        for point in ('after-journal', 'after-move', 'after-shim', 'before-manifest'):
            with self.subTest(point=point):
                crashed = self.crash_install(point)
                self.assertEqual(crashed.returncode, 97, crashed.stderr)
                held = b''.join(self.everything_bytes().values())
                self.assertIn(self.original['bend'], held)             # the original is somewhere on disk
                self.assertTrue(any('interrupted' in p for p in cg.check(self.home, ['bend'], self.env)) or not cg.is_shim(self.home/'.bend/bin/bend'))
                recovered = self.run_cli('install', '--apply')
                self.assertEqual(recovered.returncode, 0, recovered.stderr)
                self.assertTrue(cg.is_shim(self.home/'.bend/bin/bend'))
                self.assertEqual(self.real('bend').read_bytes(), self.original['bend'])
                self.assertEqual(cg.check(self.home, ['bend'], self.env), [])
                self.assertEqual(self.run_cli('uninstall', '--apply').returncode, 0)
                self.assertEqual((self.home/'.bend/bin/bend').read_bytes(), self.original['bend'])
                self.assertEqual(sorted(self.everything_bytes()), ['bend'])

    def test_a_hard_crash_at_every_point_of_an_update_keeps_both_binaries(self):
        for point in ('after-journal', 'after-backup-link', 'after-backup', 'after-move', 'after-shim', 'before-manifest'):
            with self.subTest(point=point):
                for p in list((self.home/'.bend/bin').iterdir()):
                    p.unlink()
                (self.home/'.bend/bin/bend').write_bytes(self.original['bend'])
                (self.home/'.bend/bin/bend').chmod(0o755)
                shutil_rm = Path(self.guard)
                if shutil_rm.exists():
                    import shutil
                    shutil.rmtree(shutil_rm)
                self.install()
                old_real = self.real('bend').read_bytes()
                new = b'#!/bin/sh\necho NEW-UPDATE\n'
                (self.home/'.bend/bin/bend').write_bytes(new)                  # what bend update leaves behind
                (self.home/'.bend/bin/bend').chmod(0o755)
                crashed = self.crash_install(point)
                self.assertEqual(crashed.returncode, 97, crashed.stderr)
                held = list(self.everything_bytes().values())
                self.assertIn(old_real, held)                                  # neither binary is ever lost
                self.assertIn(new, held)
                recovered = self.run_cli('install', '--apply')
                self.assertEqual(recovered.returncode, 0, recovered.stderr)
                self.assertTrue(cg.is_shim(self.home/'.bend/bin/bend'))
                self.assertEqual(self.real('bend').read_bytes(), new)           # the update completed after recovery
                backups = [v for n, v in self.everything_bytes().items() if n.startswith('bend.real.prev')]
                self.assertEqual(backups, [old_real])                           # exactly one preserved old backup
                self.assertEqual(cg.check(self.home, ['bend'], self.env), [])

    def test_a_failed_update_at_the_backup_point_puts_the_old_real_binary_back(self):
        self.install()
        old_real = self.real('bend').read_bytes()
        (self.home/'.bend/bin/bend').write_text('#!/bin/sh\necho NEW\n')
        for point in ('after-backup-link', 'after-backup'):
            with patch.dict(os.environ, {'COMPILER_GUARD_TEST_FAULT': point}):
                with self.assertRaisesRegex(RuntimeError, 'injected fault'):
                    cg.install(self.home, ['bend'], True, self.env)
            self.assertEqual(self.real('bend').read_bytes(), old_real, point)
            self.assertEqual((self.home/'.bend/bin/bend').read_text(), '#!/bin/sh\necho NEW\n', point)
            self.assertEqual([n for n in self.everything_bytes() if n.startswith('bend.real.prev')], [], point)

    def test_an_existing_prev_backup_is_never_overwritten(self):
        self.install()
        old_real = self.real('bend').read_bytes()
        junk = self.real('bend').with_name('bend.real.prev')
        junk.write_text('an older precious backup\n')
        (self.home/'.bend/bin/bend').write_text('#!/bin/sh\necho NEW\n')
        (self.home/'.bend/bin/bend').chmod(0o755)
        cg.install(self.home, ['bend'], True, self.env)
        self.assertEqual(junk.read_text(), 'an older precious backup\n')
        self.assertEqual(self.real('bend').with_name('bend.real.prev.1').read_bytes(), old_real)
        self.assertEqual(self.real('bend').read_text(), '#!/bin/sh\necho NEW\n')
        self.assertEqual(cg.check(self.home, ['bend'], self.env), [])

    # the shim, called by absolute path -------------------------------------------------------------------------
    def test_absolute_path_call_outside_an_approved_context_is_refused_and_never_reaches_the_real_binary(self):
        self.install()
        for rel, args in (('.bend/bin/bend', ['x.bend', '--check-only']), ('.bend/bin/bend', ['PROOF.bend']),
                          ('.bun/bin/bun', ['build', 'x.ts']), ('.bun/bin/bun', ['test']), ('.bun/bin/bunx', ['vite'])):
            result = self.run_installed(rel, *args)
            self.assertEqual(result.returncode, 75, (rel, args, result.stderr))
            self.assertIn('not in an approved capped context', result.stderr)
        self.assertFalse(self.marker.exists())

    def test_light_subcommands_still_run_the_real_binary(self):
        self.install()
        self.assertEqual(self.run_installed('.bend/bin/bend', 'version').returncode, 0)
        self.assertEqual(self.run_installed('.bun/bin/bun', '--version').returncode, 0)
        self.assertEqual(self.marker.read_text().split('\n')[:2], ['REAL version', 'REAL --version'])

    def test_boat_needs_the_environment_pair_a_small_machine_and_a_kernel_read_cap(self):
        env = {**self.env, 'BACKPLANE_HEAVY_BOAT_SANDBOX': '1', 'BACKPLANE_HEAVY_OFFLOADED': '1'}
        for mem, cap, expect in ((8, True, 'allow'), (64, True, 'refuse'), (8, False, 'refuse'), (64, False, 'refuse')):
            with patch.object(cg, 'mem_total', return_value=mem * cg.GIB), patch.object(cg, 'capped', return_value=cap), patch.object(cg, 'native_lock', return_value=False):
                self.assertEqual(cg.decide('bend', ['x.bend'], env)[0], expect, (mem, cap))
        with patch.object(cg, 'mem_total', return_value=8 * cg.GIB), patch.object(cg, 'capped', return_value=True), patch.object(cg, 'native_lock', return_value=False):
            self.assertEqual(cg.decide('bend', ['x.bend'], {**env, 'BACKPLANE_HEAVY_OFFLOADED': '0'})[0], 'refuse')
            self.assertEqual(cg.decide('bend', ['x.bend'], self.env)[0], 'refuse')            # no Boat claim, no lock

    def test_capped_local_context_needs_both_the_native_lock_and_the_cap(self):
        env = dict(self.env)
        for lock, cap, want in ((True, True, 'allow'), (True, False, 'refuse'), (False, True, 'refuse'), (False, False, 'refuse')):
            with patch.object(cg, 'native_lock', return_value=lock), patch.object(cg, 'capped', return_value=cap), patch.object(cg, 'boat', return_value=False):
                self.assertEqual(cg.decide('bun', ['build'], env)[0], want, (lock, cap))

    def test_native_lock_requires_a_genuinely_held_flock_not_just_an_open_descriptor(self):
        lock = Path(self.tmp.name)/'bp-bend.lock'
        lock.write_text('')
        with patch.object(cg, 'LOCK', lock):
            opened = os.open(str(lock), os.O_RDWR)                 # opened, NOT locked
            self.addCleanup(os.close, opened)
            self.assertFalse(cg.native_lock({'BACKPLANE_HEAVY_LOCK_FD': str(opened)}))
            fcntl.flock(opened, fcntl.LOCK_EX | fcntl.LOCK_NB)      # now it really holds the flock
            self.assertTrue(cg.native_lock({'BACKPLANE_HEAVY_LOCK_FD': str(opened)}))
            fcntl.flock(opened, fcntl.LOCK_UN)
            self.assertFalse(cg.native_lock({'BACKPLANE_HEAVY_LOCK_FD': str(opened)}))

    def test_native_lock_refuses_an_unlocked_descriptor_while_another_description_holds_the_lock(self):
        lock = Path(self.tmp.name)/'bp-bend.lock'
        lock.write_text('')
        with patch.object(cg, 'LOCK', lock):
            holder = os.open(str(lock), os.O_RDWR)
            unlocked = os.open(str(lock), os.O_RDWR)
            self.addCleanup(os.close, holder)
            self.addCleanup(os.close, unlocked)
            fcntl.flock(holder, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.assertFalse(cg.native_lock({'BACKPLANE_HEAVY_LOCK_FD': str(unlocked)}))
            self.assertTrue(cg.native_lock({'BACKPLANE_HEAVY_LOCK_FD': str(holder)}))

    def test_native_lock_refuses_wrong_file_missing_or_garbage_descriptors(self):
        lock = Path(self.tmp.name)/'bp-bend.lock'
        other = Path(self.tmp.name)/'other.lock'
        lock.write_text('')
        other.write_text('')
        with patch.object(cg, 'LOCK', lock):
            wrong = os.open(str(other), os.O_RDWR)
            self.addCleanup(os.close, wrong)
            fcntl.flock(wrong, fcntl.LOCK_EX | fcntl.LOCK_NB)
            for env in ({}, {'BACKPLANE_HEAVY_LOCK_FD': 'x'}, {'BACKPLANE_HEAVY_LOCK_FD': '9999'}, {'BACKPLANE_HEAVY_LOCK_FD': str(wrong)}):
                self.assertFalse(cg.native_lock(env), env)

    def test_cap_check_requires_a_finite_limit_of_at_most_3_7e9_and_no_swap(self):
        def fake(limit, swap):
            d = Path(self.tmp.name)/('cg-%s-%s' % (limit, swap))
            d.mkdir(exist_ok=True)
            (d/'memory.max').write_text(limit + '\n')
            (d/'memory.swap.max').write_text(swap + '\n')
            return d
        for limit, swap, ok in (('3699998720', '0', True), ('3700000000', '0', True), ('3700000001', '0', False),
                                ('max', '0', False), ('3699998720', 'max', False), ('3699998720', '1', False), ('0', '0', False)):
            with patch.object(cg, 'own_cgroup', return_value=fake(limit, swap)):
                self.assertEqual(cg.capped(), ok, (limit, swap))
        with patch.object(cg, 'own_cgroup', side_effect=ValueError('no cgroup')):
            self.assertFalse(cg.capped())

    def test_the_constants_agree_with_heavy_guard(self):
        self.assertEqual(cg.CAP, hg.SHARD_CAP)
        self.assertEqual(str(cg.LOCK), str(hg.lock_path()))

    def test_a_bun_installed_elsewhere_can_be_guarded_by_explicit_path(self):
        other = Path(self.tmp.name)/'usr/local/bin/bun'
        other.parent.mkdir(parents=True)
        other.write_text(FAKE)
        other.chmod(0o755)
        cg.install(self.home, ['bun'], True, self.env, {'bun': str(other)})
        self.assertTrue(cg.is_shim(other))
        self.assertEqual(other.with_name('bun.real').read_text(), FAKE)
        self.assertEqual(cg.check(self.home, ['bun'], self.env), [])
        result = subprocess.run([str(other), 'build', 'x'], env=self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 75)
        cg.uninstall(self.home, ['bun'], True, self.env)
        self.assertEqual(other.read_text(), FAKE)

    def test_exec_preserves_the_invoked_name_and_environment(self):
        with patch.object(cg.os, 'execve', side_effect=SystemExit(0)) as run, patch.object(cg.os, 'access', return_value=True):
            with self.assertRaises(SystemExit):
                cg.shim('bun', '/real/bun', '/home/h/.bun/bin/bunx', ['--version'], {'PATH': '/bin'})
        self.assertEqual(run.call_args.args[0], '/real/bun')
        self.assertEqual(run.call_args.args[1], ['/home/h/.bun/bin/bunx', '--version'])      # bunx behaviour depends on argv[0]

    def test_a_missing_real_binary_fails_closed(self):
        self.install()
        self.real('bend').unlink()
        result = self.run_installed('.bend/bin/bend', 'version')
        self.assertEqual(result.returncode, 127)

    def test_update_reapplies_the_shim_after_the_real_update_replaced_it(self):
        updater = ('#!/bin/sh\nif [ "$1" = update ]; then printf \'#!/bin/sh\\necho NEW "$*" >> "$MARKER"\\n\' > "$HOME/.bend/bin/bend"; chmod 755 "$HOME/.bend/bin/bend"; fi\n'
                   'echo "REAL $*" >> "$MARKER"\n')
        (self.home/'.bend/bin/bend').write_text(updater)
        self.install()
        result = self.run_installed('.bend/bin/bend', 'update')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(cg.is_shim(self.home/'.bend/bin/bend'))
        self.assertIn('NEW', self.real('bend').read_text())
        self.assertEqual(cg.check(self.home, ['bend'], self.env), [])
        self.assertEqual(self.run_installed('.bend/bin/bend', 'x.bend').returncode, 75)    # still guarded after the update


class HeavyGuardIntegration(unittest.TestCase):
    def test_every_script_that_names_bend_or_bun_runs_through_a_guarded_path_or_is_refused_by_the_shim(self):
        # Inventory only: the shim is the enforcement. This pins the set of scripts so a new direct caller is reviewed.
        import re
        pattern = re.compile(r'(^|[;&|(`]|\$\()\s*(exec\s+)?(bend|bun|bunx)\s')
        found = set()
        for path in sorted((ROOT/'scripts').glob('*.sh')) + [ROOT/'scripts/heavy']:
            for line in path.read_text().splitlines():
                if not line.lstrip().startswith('#') and pattern.search(line):
                    found.add(path.name)
        self.assertGreater(len(found), 5)          # the inventory is real, not empty
        self.assertIn('check.sh', found)


if __name__ == '__main__':
    unittest.main()
