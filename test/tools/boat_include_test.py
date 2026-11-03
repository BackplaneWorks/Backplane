import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

class BoatIncludeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        base = Path(self.tmp.name)
        self.project, self.fake, self.state = (base / x for x in ('project', 'fake', 'state'))
        for p in (self.project, self.fake, self.state): p.mkdir()
        subprocess.run(['git', 'init', '-q', str(self.project)], check=True)
        (self.project / 'one.txt').write_text('one')
        (self.project / 'two.txt').write_text('two')
        (self.project / '.env').write_text('secret')
        (self.project / '.gitignore').write_text('build/\n')
        (self.project / 'build').mkdir()
        (self.project / 'build' / 'ignored.txt').write_text('ignored')
        subprocess.run(['git', '-C', str(self.project), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.project), '-c', 'user.name=test', '-c', 'user.email=test@example.invalid', 'commit', '-qm', 'test'], check=True)
        (self.state / 'enabled').write_text(str(self.project) + '\n')
        limits = {'activeSandboxes': 'ACTIVE', 'maxActiveSandboxes': 5, 'canStart': True, 'subscriptionRemainingSeconds': 90000, 'creditBalanceSeconds': 2000000, 'trialComputeCapSeconds': 90000, 'creditUsedSeconds': 0, 'starts': 'STARTS', 'machineTypes': [{'type': 'default', 'available': True}]}
        (self.fake / 'limits.json').write_text(json.dumps(limits).replace('"ACTIVE"', 'ACTIVE').replace('"STARTS"', 'STARTS'))
        self.env = dict(os.environ, BOAT_FAKE_DIR=str(self.fake), BOAT_JOB_ROOT=str(self.state), BACKPLANE_BOAT=str(ROOT / 'test/tools/boat-fake'), BOAT_JOB_SCOPED='1', BACKPLANE_THREAD='include-test')
    def run_job(self, *args):
        # The runner's short-lived heartbeat can outlive its parent briefly;
        # file-backed output does not wait for that child's inherited pipe.
        with tempfile.TemporaryFile(mode='w+t') as out, tempfile.TemporaryFile(mode='w+t') as err:
            r = subprocess.run([str(ROOT / 'scripts/boat-job.sh'), '--idle', '0', '--no-setup-bend', *args, '--', 'true'], cwd=self.project, env=self.env, text=True, stdout=out, stderr=err, timeout=30)
            out.seek(0); err.seek(0)
            r.stdout, r.stderr = out.read(), err.read()
            return r
    def test_exact_inventory_and_unrelated_untracked_file(self):
        (self.project / 'later-archive.tar').write_text('must not travel')
        r = self.run_job('--include-file', 'one.txt', '--include-file', 'two.txt')
        self.assertEqual(r.returncode, 0, r.stderr)
        manifests = list((self.state / 'jobs').glob('*/manifest.sha256'))
        manifest = manifests[0].read_text()
        self.assertIn('./one.txt', manifest); self.assertIn('./two.txt', manifest)
        self.assertNotIn('later-archive.tar', manifest); self.assertNotIn('.env', manifest)
    def test_missing_excluded_ignored_and_secret_fail_before_create(self):
        for args in (('--include-file', 'missing'), ('--include-file', 'one.txt', '--exclude', 'one*'), ('--include-file', 'build/ignored.txt'), ('--include-file', '.env'), ('--include-file', '../one.txt')):
            r = self.run_job(*args)
            self.assertEqual(r.returncode, 76, (args, r.stdout, r.stderr))
        self.assertFalse((self.fake / 'newcalls').exists())
    def test_deleted_tracked_file_fails(self):
        (self.project / 'one.txt').unlink()
        r = self.run_job('--include-file', 'one.txt')
        self.assertEqual(r.returncode, 76, r.stderr)
        self.assertFalse((self.fake / 'newcalls').exists())

if __name__ == '__main__': unittest.main()
