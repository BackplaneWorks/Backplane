import json, os, subprocess, sys, tempfile, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import importlib.util
spec = importlib.util.spec_from_file_location('lowmem', ROOT / 'scripts' / 'lowmem-build.py')
lm = importlib.util.module_from_spec(spec); spec.loader.exec_module(lm)

SRC = {'head': 'a' * 40, 'tree': 'b' * 40, 'clean': True}
TOOLS = {'bend': {'version': '1', 'sha256': 'c' * 64}, 'bun': None}


def receipt(**kw):
    r = {'id': 'hub', 'exit': 0, 'cap_bytes': lm.CAP_BYTES, 'peak_bytes': 2 * lm.GIB, 'over_cap': False,
         'swap_activity': False, 'source': SRC, 'tools': TOOLS, 'outputs': {'hub.js': 'x'}}
    r.update(kw)
    return r


JOB = {'id': 'hub', 'kind': 'hub'}
REF = {'outputs': {'hub': {'hub.js': 'x'}}}


class Judge(unittest.TestCase):
    def j(self, r, ref=REF, src=SRC, tools=TOOLS):
        return lm.judge(JOB, r, ref, src, tools, lm.CAP_BYTES)

    def test_pass(self):
        self.assertEqual(self.j(receipt()), [])

    def test_over_cap(self):
        self.assertTrue(any('OVER CAP' in w for w in self.j(receipt(peak_bytes=lm.CAP_BYTES + 1))))
        self.assertTrue(any('OVER CAP' in w for w in self.j(receipt(over_cap=True))))

    def test_wrong_cap_swap_exit(self):
        self.assertTrue(self.j(receipt(cap_bytes=8 * lm.GIB)))
        self.assertTrue(any('SWAP' in w for w in self.j(receipt(swap_activity=True))))
        self.assertTrue(self.j(receipt(exit=1)))
        self.assertTrue(self.j(None))

    def test_bytes_differ_or_missing_reference(self):
        self.assertTrue(any('BYTE-IDENTICAL' in w for w in self.j(receipt(outputs={'hub.js': 'y'}))))
        self.assertTrue(any('no reference' in w for w in self.j(receipt(), ref={'outputs': {}})))

    def test_provenance(self):
        self.assertTrue(self.j(receipt(source={**SRC, 'tree': 'd' * 40})))
        self.assertTrue(self.j(receipt(tools={'bend': {'version': '2', 'sha256': 'c' * 64}, 'bun': None})))
        dirty = {**SRC, 'clean': False}
        self.assertTrue(self.j(receipt(source=dirty), src=dirty))


class Worker(unittest.TestCase):
    def run_worker(self, tmp, cap, code, extra=()):
        out = Path(tmp) / 'o'
        p = subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), 'worker', '--id', 'hub', '--out', str(out),
                            '--cap-bytes', str(cap), '--poll', '0.02', '--', sys.executable, '-c', code], capture_output=True, text=True)
        return p, json.loads((out / 'hub.receipt.json').read_text())

    def test_under_cap_records_peak_and_hash(self):
        with tempfile.TemporaryDirectory() as t:
            code = "import sys;open(sys.argv[0] if 0 else '%s/o/hub.js','w').write('hi')" % t
            p, r = self.run_worker(t, 2 * lm.GIB, code)
            self.assertEqual(p.returncode, 0, p.stderr)
            self.assertEqual(r['outputs'], {'hub.js': lm.sha(b'hi')})
            self.assertGreater(r['peak_bytes'], 0)

    def test_over_cap_is_killed(self):
        with tempfile.TemporaryDirectory() as t:
            code = "b=bytearray(300*1024*1024)\nimport time\nfor i in range(0,len(b),4096): b[i]=1\ntime.sleep(30)"
            p, r = self.run_worker(t, 100 * 1024 * 1024, code)
            self.assertEqual(p.returncode, 1)
            self.assertTrue(r['over_cap'])


class Cli(unittest.TestCase):
    def cli(self, *a):
        return subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), *a], capture_output=True, text=True)

    def test_dry_run_launches_nothing(self):
        p = self.cli('run', '--dry-run', '--jobs', 'hub,web,mobile')
        self.assertEqual(p.returncode, 0, p.stderr)
        for k in ('hub', 'web', 'mobile'):
            self.assertIn(k, p.stdout)
        self.assertIn('--size default', p.stdout)
        self.assertNotIn('xlarge', p.stdout.replace('never xlarge', ''))

    def test_unqualified_is_loud_exit_20_no_silent_fallback(self):
        with tempfile.TemporaryDirectory() as t:
            p = self.cli('run', '--jobs', 'hub', '--workdir', t, '--runner', 'local')
            self.assertEqual(p.returncode, 20)
            self.assertIn('NOT USED', p.stderr)
            self.assertIn('No silent switch', p.stderr)

    def test_explicit_fallback_is_announced(self):
        with tempfile.TemporaryDirectory() as t:
            p = self.cli('run', '--jobs', 'hub', '--workdir', t, '--allow-large-fallback', '--fallback-cmd', 'true')
            self.assertEqual(p.returncode, 0)
            self.assertIn('EXPLICIT FALLBACK', p.stderr)
            rep = json.loads(next(Path(t).glob('lowmem-*/report.json')).read_text())
            self.assertTrue(rep['fallback'])

    def test_release_refuses_local_runner(self):
        self.assertEqual(self.cli('run', '--jobs', 'hub', '--release', '--runner', 'local').returncode, 2)

    def test_end_to_end_local_fake_compiler(self):
        # a fake "bend" on PATH proves collection, identity and the gates without a Boat
        with tempfile.TemporaryDirectory() as t:
            t = Path(t); bindir = t / 'bin'; bindir.mkdir()
            (bindir / 'bend').write_text('#!/bin/sh\nif [ "$1" = --version ]; then echo fake; exit 0; fi\nprintf "%s" "${FAKE_OUT:-same}" > "$3"\n')
            (bindir / 'bend').chmod(0o755)
            env = {**os.environ, 'PATH': str(bindir) + os.pathsep + os.environ['PATH']}
            q = t / 'q.json'; q.write_text('{"qualified": true}')
            ref = t / 'ref.json'
            # the reference as the current compiler would emit it
            src = lm.source_identity()
            if not src['clean']:
                self.skipTest('needs a clean tree')
            ref.write_text(json.dumps({'outputs': {'hub': {'hub.js': lm.sha(b'same')}},
                                       'tools': None}))
            base = ['run', '--jobs', 'hub', '--runner', 'local', '--workdir', str(t / 'w'), '--qualification', str(q), '--reference', str(ref)]
            ok = subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), *base], capture_output=True, text=True, env=env)
            self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
            self.assertIn('end-to-end wall', ok.stdout)
            bad = subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), *base], capture_output=True, text=True,
                                 env={**env, 'FAKE_OUT': 'different'})
            self.assertEqual(bad.returncode, 1)
            self.assertIn('NOT BYTE-IDENTICAL', bad.stdout)


if __name__ == '__main__':
    unittest.main()
