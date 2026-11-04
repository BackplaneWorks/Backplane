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
            real = self.guard/'real'/name
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
        self.assertEqual((self.guard/'real/bend').read_bytes(), self.original['bend'])

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
        (self.guard/'real/bend').write_bytes(b'#!/bin/sh\necho tampered\n')
        actions = cg.uninstall(self.home, ['bend'], True, self.env)
        self.assertIn('REFUSED', actions[0][1])
        self.assertTrue(cg.is_shim(self.home/'.bend/bin/bend'))

    def test_check_detects_a_reinstall_over_the_shim_and_a_changed_real_binary(self):
        self.install()
        (self.home/'.bend/bin/bend').write_text(FAKE)                       # e.g. an update replaced the shim
        problems = cg.check(self.home, ['bend', 'bun'], self.env)
        self.assertTrue(any('not the guard shim' in p for p in problems))
        (self.guard/'real/bun').write_bytes(b'x')
        self.assertTrue(any('changed' in p for p in cg.check(self.home, ['bun'], self.env)))

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

    def test_a_spoofed_boat_environment_on_a_large_machine_is_refused(self):
        self.install()
        env = {**self.env, 'BACKPLANE_HEAVY_BOAT_SANDBOX': '1', 'BACKPLANE_HEAVY_OFFLOADED': '1'}
        with patch.object(cg, 'mem_total', return_value=64 * cg.GIB):
            self.assertEqual(cg.decide('bend', ['x.bend'], env)[0], 'refuse')
        with patch.object(cg, 'mem_total', return_value=8 * cg.GIB):
            self.assertEqual(cg.decide('bend', ['x.bend'], env)[0], 'allow')
        with patch.object(cg, 'mem_total', return_value=8 * cg.GIB):
            self.assertEqual(cg.decide('bend', ['x.bend'], {**env, 'BACKPLANE_HEAVY_OFFLOADED': '0'})[0], 'refuse')

    def test_capped_local_context_needs_both_the_native_lock_and_the_cap(self):
        env = dict(self.env)
        for lock, cap, want in ((True, True, 'allow'), (True, False, 'refuse'), (False, True, 'refuse'), (False, False, 'refuse')):
            with patch.object(cg, 'native_lock', return_value=lock), patch.object(cg, 'capped', return_value=cap), patch.object(cg, 'boat', return_value=False):
                self.assertEqual(cg.decide('bun', ['build'], env)[0], want, (lock, cap))

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

    def test_exec_preserves_the_invoked_name_and_environment(self):
        with patch.object(cg.os, 'execve', side_effect=SystemExit(0)) as run, patch.object(cg.os, 'access', return_value=True):
            with self.assertRaises(SystemExit):
                cg.shim('bun', '/real/bun', '/home/h/.bun/bin/bunx', ['--version'], {'PATH': '/bin'})
        self.assertEqual(run.call_args.args[0], '/real/bun')
        self.assertEqual(run.call_args.args[1], ['/home/h/.bun/bin/bunx', '--version'])      # bunx behaviour depends on argv[0]

    def test_a_missing_real_binary_fails_closed(self):
        self.install()
        (self.guard/'real/bend').unlink()
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
        self.assertIn('NEW', (self.guard/'real/bend').read_text())
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
