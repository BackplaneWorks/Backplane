import json, os, subprocess, sys, tempfile, time, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import importlib.util
spec = importlib.util.spec_from_file_location('lowmem', ROOT / 'scripts' / 'lowmem-build.py')
lm = importlib.util.module_from_spec(spec); spec.loader.exec_module(lm)

import argparse
from unittest.mock import patch

H = lambda c: c * 64
SRC = {'head': 'a' * 40, 'tree': 'b' * 40, 'clean': True, 'closure_sha256': H('9')}
TOOLS = {'bend': {'version': '1', 'sha256': H('c')}, 'bun': {'version': '1.2.3', 'sha256': H('d')}}
CAP = lm.CAP_BYTES
CMD = ['bend', 'src/server/main.bend', '-o', '{out}/hub.js']
PROT = {n: H('e') for n in lm.PROTECTED}
QUAL = {'schema': 3, 'qualified': True, 'source': SRC, 'tools': TOOLS, 'env': lm.PINNED_ENV,
        'protected': {n: {'path': '/p/' + n, 'sha256': h} for n, h in PROT.items()},
        'jobs': {k: {'cmd': CMD, 'phases': lm.PHASES, 'timeout_seconds': 3000,
                     'artifacts': {'hub.js': {'sha256': H('1'), 'bytes': 100}}} for k in ('hub', 'web', 'mobile')}}


def enforcement(**kw):
    e = {'executor': 'cgroup-v2', 'cgroup': '/x', 'overridden': False, 'memory_max': CAP, 'memory_swap_max': 0,
         'memory_peak': 2 * lm.GIB, 'swap_peak': 0, 'oom': 0, 'oom_kill': 0, 'oom_group_kill': 0}
    e.update(kw)
    return e


def receipt(**kw):
    r = {'schema': 3, 'id': 'hub', 'exit': 0, 'cap_bytes': CAP, 'peak_bytes': 2 * lm.GIB, 'enforcement': enforcement(),
         'controller': enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=100 * 1024 ** 2),
         'swap_activity': False, 'boat_sandbox': True, 'mem_total_bytes': 8 * lm.GIB, 'source': SRC, 'tools': TOOLS,
         'outputs': {'hub.js': H('1')}, 'output_bytes': {'hub.js': 100}, 'cmd_template': CMD, 'timeout_seconds': 3000,
         'timed_out': False, 'flock': True, 'phases': list(lm.PHASES), 'env': lm.PINNED_ENV, 'protected': PROT}
    r.update(kw)
    return r


JOB = {'id': 'hub', 'kind': 'hub'}
REF = {'outputs': {'hub': {'hub.js': H('1')}}}


def qual(**kw):
    q = json.loads(json.dumps(QUAL)); q.update(kw); return q


class Judge(unittest.TestCase):
    def j(self, r, ref=REF, src=SRC, q=None):
        return lm.judge(JOB, r, ref, src, q or QUAL, CAP)

    def test_pass(self):
        self.assertEqual(self.j(receipt()), [])

    def test_kernel_cap_required_exactly(self):
        for e in (enforcement(memory_max='max'), enforcement(memory_max=None), enforcement(memory_max=CAP + 4096),
                  enforcement(memory_swap_max='max'), enforcement(memory_swap_max=1), enforcement(memory_peak=None),
                  enforcement(memory_peak=CAP + 1), enforcement(oom_kill=1), enforcement(oom=1), enforcement(overridden=True),
                  enforcement(executor='rss-poller'), enforcement(swap_peak=4096), None):
            self.assertTrue(self.j(receipt(enforcement=e)), e)

    def test_controller_cap_enforced_and_sum_under_4gib(self):
        self.assertEqual(lm.COMPILER_CAP, 3699998720); self.assertEqual(lm.CONTROLLER_CAP, 536870912)
        self.assertLess(lm.COMPILER_CAP + lm.CONTROLLER_CAP, 4 * lm.GIB)
        for c in (None, enforcement(memory_max='max'), enforcement(memory_max=lm.CONTROLLER_CAP, memory_swap_max=1),
                  enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=lm.CONTROLLER_CAP + 1)):
            self.assertTrue(self.j(receipt(controller=c)), c)

    def test_not_a_normal_boat_job(self):
        self.assertTrue(self.j(receipt(boat_sandbox=False)))
        self.assertTrue(self.j(receipt(mem_total_bytes=64 * lm.GIB)))
        self.assertTrue(self.j(receipt(schema=2)))

    def test_exit_swap_missing(self):
        for r in (receipt(exit=1), receipt(exit=None), receipt(exit=75), receipt(swap_activity=True), None):
            self.assertTrue(self.j(r))

    def test_timeout_flock_phases(self):
        for r in (receipt(timed_out=True), receipt(timed_out=None), receipt(timeout_seconds=None), receipt(timeout_seconds=0),
                  receipt(timeout_seconds=10 ** 6), receipt(flock=False), receipt(phases=lm.PHASES[:-1]),
                  receipt(phases=lm.PHASES[::-1]), receipt(phases=[])):
            self.assertTrue(self.j(r), r['phases'] if r else r)

    def test_qualification_timeout_must_match_receipt(self):
        q = qual(); q['jobs']['hub']['timeout_seconds'] = 600
        self.assertTrue(self.j(receipt(), q=q))

    def test_source_closure_commit_tree_mismatch(self):
        for k, v in (('closure_sha256', H('8')), ('tree', 'd' * 40), ('head', 'd' * 40)):
            self.assertTrue(self.j(receipt(source={**SRC, k: v})), k)
        q = qual(); q['source']['closure_sha256'] = H('8')
        self.assertTrue(self.j(receipt(), q=q))
        dirty = {**SRC, 'clean': False}
        self.assertTrue(self.j(receipt(source=dirty), src=dirty, q=qual(source=dirty)))

    def test_compiler_and_bun_hashes(self):
        for k in ('bend', 'bun'):
            self.assertTrue(self.j(receipt(tools={**TOOLS, k: {**TOOLS[k], 'sha256': H('0')}})), k)
            self.assertTrue(self.j(receipt(tools={**TOOLS, k: None})), k)

    def test_protected_inputs(self):
        for n in lm.PROTECTED:
            self.assertTrue(self.j(receipt(protected={**PROT, n: H('0')})), n)
        self.assertTrue(self.j(receipt(protected={k: v for k, v in PROT.items() if k != 'base'})))

    def test_flags_and_environment(self):
        self.assertTrue(self.j(receipt(env={**lm.PINNED_ENV, 'BEND_NO_TELEMETRY': '0'})))
        self.assertTrue(self.j(receipt(cmd_template=CMD + ['--extra'])))

    def test_final_artifact_size_and_sha(self):
        self.assertTrue(self.j(receipt(output_bytes={'hub.js': 99})))
        self.assertTrue(self.j(receipt(outputs={'hub.js': H('2')})))
        self.assertTrue(self.j(receipt(outputs={'hub.js': H('1'), 'x.js': H('1')}, output_bytes={'hub.js': 100, 'x.js': 1})))

    def test_bytes_differ_from_reference(self):
        q = qual(); q['jobs']['hub']['artifacts']['hub.js']['sha256'] = H('2')
        r = receipt(outputs={'hub.js': H('2')})
        self.assertTrue(any('BYTE-IDENTICAL' in w for w in self.j(r, q=q)))
        self.assertTrue(any('no reference' in w for w in self.j(receipt(), ref={'outputs': {}})))


class Qualified(unittest.TestCase):
    def q(self, mutate=None):
        q = qual()
        if mutate: mutate(q)
        with tempfile.TemporaryDirectory() as t:
            f = Path(t) / 'q.json'; f.write_text(json.dumps(q))
            return lm.qualified(str(f), SRC)

    def test_valid(self):
        self.assertTrue(self.q()[0])

    def test_bare_true_is_insufficient(self):
        self.assertFalse(self.q(lambda q: [q.pop(k) for k in list(q) if k not in ('qualified',)])[0])

    def test_each_mismatch_refuses(self):
        for m in (lambda q: q['source'].update(closure_sha256=H('8')), lambda q: q['source'].update(tree='d' * 40),
                  lambda q: q['source'].update(head='d' * 40), lambda q: q['tools'].pop('bun'),
                  lambda q: q['tools']['bend'].pop('sha256'), lambda q: q['protected'].pop('safe.ts'),
                  lambda q: q['protected']['base'].update(sha256='z'),
                  lambda q: q.update(env={}), lambda q: q['jobs'].pop('mobile'),
                  lambda q: q['jobs']['web'].update(artifacts={}), lambda q: q['jobs']['web']['artifacts']['hub.js'].update(bytes=0),
                  lambda q: q['jobs']['web']['artifacts']['hub.js'].update(sha256='short'),
                  lambda q: q['jobs']['web'].update(phases=[]), lambda q: q['jobs']['web'].update(timeout_seconds=0),
                  lambda q: q['jobs']['web'].update(cmd=[]), lambda q: q.update(qualified=False), lambda q: q.update(schema=2)):
            self.assertFalse(self.q(m)[0])
        self.assertFalse(lm.qualified(None, SRC)[0])
        self.assertFalse(lm.qualified('/nonexistent', SRC)[0])


class Producer(unittest.TestCase):
    """The reference comes only from a saved verified producer emission."""
    def make(self, t, **mut):
        d = Path(t); (d / 'web').mkdir(); (d / 'logs').mkdir()
        (d / 'server.js').write_text('raw'); (d / 'web/chunk-a.js').write_text('w')
        (d / 'SOURCE.json').write_text(json.dumps({'commit': 'a' * 40, 'tree': 'b' * 40, 'members': {}}))
        (d / 'PHASES.json').write_text(json.dumps([{'phase': p, 'exitCode': 0} for p in ('entries', 'server', 'web')]))
        files = {p.relative_to(d).as_posix(): {'sha256': lm.sha(p.read_bytes()), 'bytes': p.stat().st_size}
                 for p in d.rglob('*') if p.is_file()}
        e = {'schema': 1, 'kind': 'backplane-secret-free-js-emission', 'status': 'VERIFIED', 'channel': 'release', 'published': False,
             'sourceCommit': 'a' * 40, 'sourceTree': 'b' * 40, 'compiler': {'sha256': H('c')}, 'files': files}
        e.update(mut)
        (d / 'EMISSION.json').write_text(json.dumps(e))
        return d

    def test_valid_and_pinned(self):
        with tempfile.TemporaryDirectory() as t:
            d = self.make(t)
            ref = lm.reference_from_emission(d, lm.sha(b'raw'))
            self.assertEqual(ref['outputs']['hub'], {'hub.js': lm.sha(b'raw')})
            self.assertEqual(ref['source']['head'], 'a' * 40)

    def test_each_tamper_refuses(self):
        for mut in ({'status': 'DRAFT'}, {'kind': 'x'}, {'published': True}, {'sourceCommit': 'short'}):
            with tempfile.TemporaryDirectory() as t:
                with self.assertRaises(ValueError): lm.reference_from_emission(self.make(t, **mut))
        with tempfile.TemporaryDirectory() as t:
            d = self.make(t); (d / 'server.js').write_text('tampered')
            with self.assertRaises(ValueError): lm.reference_from_emission(d)
        with tempfile.TemporaryDirectory() as t:
            d = self.make(t); (d / 'extra').write_text('x')
            with self.assertRaises(ValueError): lm.reference_from_emission(d)
        with tempfile.TemporaryDirectory() as t:
            d = self.make(t)
            with self.assertRaises(ValueError): lm.reference_from_emission(d, H('0'))
        with tempfile.TemporaryDirectory() as t:
            d = self.make(t); (d / 'PHASES.json').write_text('[]')
            e = json.loads((d / 'EMISSION.json').read_text())
            e['files']['PHASES.json'] = {'sha256': lm.sha(b'[]'), 'bytes': 2}; (d / 'EMISSION.json').write_text(json.dumps(e))
            with self.assertRaises(ValueError): lm.reference_from_emission(d)

    @unittest.skipUnless(Path('/home/h/.local/state/backplane/boat/jobs/j20261103173955-1133029/artifacts/rc-emission').is_dir(), 'frozen33ce not here')
    def test_frozen33ce(self):
        ref = lm.reference_from_emission('/home/h/.local/state/backplane/boat/jobs/j20261103173955-1133029/artifacts/rc-emission',
                                         'ca921838988fe844f7e1bc3f0040d5d30f4d9d139facecd76014c17c0c94185f')
        self.assertEqual(ref['source']['head'][:4], '33ce')


class ProofResults(unittest.TestCase):
    """The planner's authenticated shard semantics and the all-shards join decide a proof pass."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.t = Path(self.tmp.name)
        self.jobs = [{'id': 'proof-s%d' % i, 'kind': 'proof', 'shard': 's%d' % i, 'identity': 'i' * 64} for i in range(2)]

    def results(self, docs):
        out = {}
        for j, doc in zip(self.jobs, docs):
            d = self.t / j['id']; d.mkdir(exist_ok=True)
            data = json.dumps(doc).encode()
            (d / 'result.json').write_bytes(data)
            out[j['id']] = (0, {'outputs': {'result.json': lm.sha(data)}}, d)
        return out

    class PS:
        def __init__(self, ok=True): self.ok = ok
        def verify(self, root, folder): return {'shards': [1, 2]}
        def join(self, manifest, folder, receipts):
            if not self.ok or len(receipts) != 2: raise ValueError('missing/extra shard receipts')
            return {'state': 'passed'}

    def good(self, i): return {'shard': 's%d' % i, 'manifest_sha256': 'i' * 64, 'state': 'passed'}

    def check(self, docs, ps=None, tamper=None):
        res = self.results(docs)
        if tamper: tamper(res)
        return lm.validate_proof(self.jobs, res, self.t, ps or self.PS())

    def test_all_good(self):
        self.assertEqual(self.check([self.good(0), self.good(1)]), {'proof-s0': [], 'proof-s1': []})

    def test_forged_non_passing_receipt(self):
        out = self.check([self.good(0), {**self.good(1), 'state': 'failed'}])
        self.assertTrue(out['proof-s1']); self.assertFalse(out['proof-s0'] and False)
        self.assertTrue(all(v for v in out.values()), 'the join must refuse every shard')

    def test_wrong_identity_or_shard(self):
        self.assertTrue(self.check([self.good(0), {**self.good(1), 'manifest_sha256': 'z' * 64}])['proof-s1'])
        self.assertTrue(self.check([self.good(0), {**self.good(0)}])['proof-s1'])

    def test_bytes_not_those_the_worker_hashed(self):
        def tamper(res):
            (res['proof-s1'][2] / 'result.json').write_text(json.dumps(self.good(1)) + ' ')
        self.assertTrue(self.check([self.good(0), self.good(1)], tamper=tamper)['proof-s1'])

    def test_missing_shard_result_and_failed_join(self):
        def drop(res): (res['proof-s1'][2] / 'result.json').unlink()
        out = self.check([self.good(0), self.good(1)], tamper=drop)
        self.assertTrue(out['proof-s1']); self.assertTrue(out['proof-s0'])
        self.assertTrue(all(self.check([self.good(0), self.good(1)], ps=self.PS(False)).values()))

    def test_judge_never_passes_unvalidated_proof(self):
        job = {'id': 'proof-s0', 'kind': 'proof'}
        r = receipt(id='proof-s0')
        self.assertTrue(lm.judge(job, r, REF, SRC, QUAL, CAP))
        self.assertEqual(lm.judge(job, r, REF, SRC, QUAL, CAP, proof=[]), [])


class Worker(unittest.TestCase):
    """The worker only trusts limits the kernel reports from inside the cgroup (mocked here; no cgroup is created)."""

    def run_worker(self, tmp, compiler, controller=None, code=0, write=True, flock=True, timed_out=False):
        a = argparse.Namespace(id='hub', out=str(Path(tmp) / 'o'), cap_bytes=CAP, cmd=CMD, copy=None, no_scope=True,
                               timeout=3000, protect=['%s=%s' % (n, __file__) for n in lm.PROTECTED])
        def capped(cmd, cap, report, timeout):
            if write: (Path(tmp) / 'o' / 'hub.js').write_text('hi')
            return code, compiler, timed_out
        ctl = controller or enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=1000)
        with patch.object(lm, 'run_capped', capped), patch.object(lm, 'cgroup_dir', return_value=(Path(tmp), False)), \
                patch.object(lm, 'read_enforcement', return_value=ctl), patch.object(lm, 'source_identity', return_value=SRC), \
                patch.object(lm, 'tool_identity', return_value=TOOLS), patch.object(lm, 'native_flock', return_value=flock):
            rc = lm.worker(a)
        return rc, json.loads((Path(a.out) / 'hub.receipt.json').read_text())

    def test_enforced_cgroups_recorded(self):
        with tempfile.TemporaryDirectory() as t:
            rc, r = self.run_worker(t, enforcement(memory_peak=123456789))
            self.assertEqual(rc, 0)
            self.assertEqual(r['peak_bytes'], 123456789)
            self.assertEqual(r['outputs'], {'hub.js': lm.sha(b'hi')})
            self.assertEqual(r['output_bytes'], {'hub.js': 2})
            self.assertEqual(r['phases'], lm.PHASES)
            self.assertEqual(r['controller']['memory_max'], lm.CONTROLLER_CAP)
            self.assertEqual(r['timeout_seconds'], 3000)
            self.assertEqual(sorted(r['protected']), sorted(lm.PROTECTED))

    def test_compiler_without_kernel_cap_fails(self):
        for e in (None, enforcement(memory_max='max'), enforcement(memory_swap_max='max'), enforcement(oom_kill=1),
                  enforcement(memory_peak=CAP + 1)):
            with tempfile.TemporaryDirectory() as t:
                self.assertEqual(self.run_worker(t, e)[0], 1, e)

    def test_nonzero_no_output_timeout_or_no_flock_fail(self):
        for kw in ({'code': 1}, {'write': False}, {'timed_out': True, 'code': None}, {'flock': False}):
            with tempfile.TemporaryDirectory() as t:
                self.assertEqual(self.run_worker(t, enforcement(), **kw)[0], 1, kw)

    def test_no_flock_never_runs_the_command(self):
        with tempfile.TemporaryDirectory() as t:
            rc, r = self.run_worker(t, enforcement(), flock=False)
            self.assertIsNone(r['exit']); self.assertNotIn('command', r['phases'])

    def test_unenforced_controller_fails(self):
        with tempfile.TemporaryDirectory() as t:
            self.assertEqual(self.run_worker(t, enforcement(), controller=enforcement(memory_max='max'))[0], 1)

    def test_shim_does_not_run_command_without_cap(self):
        with tempfile.TemporaryDirectory() as t:
            marker, rep = Path(t) / 'ran', Path(t) / 'rep.json'
            a = argparse.Namespace(cap_bytes=CAP, report=str(rep), cmd=['touch', str(marker)])
            with patch.object(lm, 'cgroup_dir', return_value=(Path(t), False)):
                self.assertEqual(lm.shim(a), 1)
            self.assertFalse(marker.exists())

    def test_real_timeout_kills_and_never_qualifies(self):
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'late'
            with patch.object(lm, 'systemd_scope', lambda cap, argv: ['sh', '-c', 'sleep 5; touch %s' % marker]):
                code, enf, timed_out = lm.run_capped(['x'], CAP, Path(t) / 'r.json', 0.3)
            self.assertTrue(timed_out); self.assertIsNone(code)
            time.sleep(0.2); self.assertFalse(marker.exists())


def real_scope_works():
    try:
        return subprocess.run(['systemd-run', '--user', '--scope', '-q', '-p', 'MemoryMax=64M', '-p', 'MemorySwapMax=0', 'true'],
                              capture_output=True, timeout=20).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


@unittest.skipUnless(real_scope_works(), 'needs a systemd user scope with the memory controller')
class RealScope(unittest.TestCase):
    """Negative controls against the actual kernel, with a small cap and a private lock file (never the shared bend lock)."""
    SMALL = 64 * 1024 * 1024

    def test_limits_read_back_from_inside_the_scope(self):
        with tempfile.TemporaryDirectory() as t:
            code, enf, timed_out = lm.run_capped([sys.executable, '-c', 'pass'], self.SMALL, Path(t) / 'r.json', 60)
        self.assertEqual(code, 0); self.assertFalse(timed_out)
        self.assertEqual((enf['memory_max'], enf['memory_swap_max']), (self.SMALL, 0))
        self.assertGreater(enf['memory_peak'], 0)
        self.assertEqual(lm.enforcement_problems(enf, self.SMALL), [])

    def test_a_memory_hog_is_killed_and_never_qualifies(self):
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'finished'
            hog = "b=bytearray(400*1024*1024)\nfor i in range(0,len(b),4096): b[i]=1\nopen(%r,'w')" % str(marker)
            code, enf, timed_out = lm.run_capped([sys.executable, '-c', hog], self.SMALL, Path(t) / 'r.json', 60)
            self.assertFalse(marker.exists())
        self.assertTrue(code != 0 or enf is not None)
        self.assertTrue(code is None or code != 0 or lm.enforcement_problems(enf, self.SMALL))

    def test_shim_outside_a_capped_scope_refuses_to_run(self):
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'ran'
            p = subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), 'shim', '--cap-bytes', str(self.SMALL),
                                '--report', str(Path(t) / 'r.json'), '--', 'touch', str(marker)], capture_output=True, text=True)
            self.assertNotEqual(p.returncode, 0); self.assertFalse(marker.exists())

    def test_whole_worker_without_admission_fails_closed_and_runs_nothing(self):
        env = {k: v for k, v in os.environ.items() if k != 'BACKPLANE_HEAVY_LOCK_FD'}
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'ran'
            p = subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), 'worker', '--id', 'hub', '--out', str(Path(t) / 'o'),
                                '--', 'touch', str(marker)], capture_output=True, text=True, env=env, cwd=ROOT)
            r = json.loads((Path(t) / 'o/hub.receipt.json').read_text())
            self.assertEqual(p.returncode, 1); self.assertFalse(marker.exists())
            self.assertFalse(r['flock']); self.assertIsNone(r['exit']); self.assertNotIn('command', r['phases'])
            self.assertEqual(r['controller']['memory_max'], lm.CONTROLLER_CAP)  # the controller was really capped

    def test_inherited_flock_survives_the_scope_and_is_refused_without_it(self):
        import fcntl
        spec = importlib.util.spec_from_file_location('heavy_guard', ROOT / 'scripts/heavy-guard.py')
        guard = importlib.util.module_from_spec(spec); spec.loader.exec_module(guard)
        with tempfile.TemporaryDirectory() as t:
            lock = Path(t) / 'lock'
            fd = os.open(lock, os.O_CREAT | os.O_RDWR); self.addCleanup(os.close, fd)
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB); os.set_inheritable(fd, True)
            code = ("import importlib.util as u;from pathlib import Path;s=u.spec_from_file_location('g',%r);g=u.module_from_spec(s);"
                    "s.loader.exec_module(g);print(g.inherited_lock(Path(%r)) is not None)" % (str(ROOT / 'scripts/heavy-guard.py'), str(lock)))
            def inside(env):
                return subprocess.run(lm.systemd_scope(lm.CONTROLLER_CAP, ['--', sys.executable, '-c', code]), env=env,
                                      capture_output=True, text=True, pass_fds=(fd,)).stdout.strip()
            self.assertEqual(inside({**os.environ, 'BACKPLANE_HEAVY_LOCK_FD': str(fd)}), 'True')
            self.assertEqual(inside({k: v for k, v in os.environ.items() if k != 'BACKPLANE_HEAVY_LOCK_FD'}), 'False')
            self.assertEqual(inside({**os.environ, 'BACKPLANE_HEAVY_LOCK_FD': '2'}), 'False')


class Cli(unittest.TestCase):
    def cli(self, *a):
        return subprocess.run([sys.executable, str(ROOT / 'scripts/lowmem-build.py'), *a], capture_output=True, text=True)

    def test_dry_run_launches_nothing(self):
        p = self.cli('run', '--dry-run', '--jobs', 'hub,web,mobile')
        self.assertEqual(p.returncode, 0, p.stderr)
        for k in ('hub', 'web', 'mobile'):
            self.assertIn(k, p.stdout)
        self.assertIn('--size default', p.stdout)

    def test_unqualified_is_loud_exit_20_no_silent_fallback(self):
        with tempfile.TemporaryDirectory() as t:
            p = self.cli('run', '--jobs', 'hub', '--workdir', t, '--runner', 'local')
            self.assertEqual(p.returncode, 20)
            self.assertIn('NOT USED', p.stderr)
            self.assertIn('No silent switch', p.stderr)

    def test_qualification_for_another_tree_is_unqualified(self):
        with tempfile.TemporaryDirectory() as t:
            q = Path(t) / 'q.json'
            q.write_text(json.dumps({'qualified': True, 'schema': 3, 'source': {'tree': '0' * 40}, 'tools': TOOLS}))
            p = self.cli('run', '--jobs', 'hub', '--workdir', t, '--runner', 'local', '--qualification', str(q))
            self.assertEqual(p.returncode, 20)
            self.assertIn('NOT USED', p.stderr)

    def test_explicit_fallback_is_announced(self):
        with tempfile.TemporaryDirectory() as t:
            p = self.cli('run', '--jobs', 'hub', '--workdir', t, '--allow-large-fallback', '--fallback-cmd', 'true')
            self.assertEqual(p.returncode, 0)
            self.assertIn('EXPLICIT FALLBACK', p.stderr)
            rep = json.loads(next(Path(t).glob('lowmem-*/report.json')).read_text())
            self.assertTrue(rep['fallback'])

    def test_release_needs_boat_and_all_three(self):
        self.assertEqual(self.cli('run', '--jobs', 'hub,web,mobile', '--release', '--runner', 'local').returncode, 2)
        self.assertEqual(self.cli('run', '--jobs', 'hub', '--release').returncode, 2)


if __name__ == '__main__':
    unittest.main()
