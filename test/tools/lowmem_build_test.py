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
         'timed_out': False, 'flock': True, 'flock_path': lm.REAL_LOCK, 'phases': list(lm.PHASES), 'env': lm.PINNED_ENV, 'protected': PROT}
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
                  receipt(timeout_seconds=10 ** 6), receipt(flock=False), receipt(flock_path='/tmp/other.lock'), receipt(phases=lm.PHASES[:-1]),
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
    """The worker only trusts limits the kernel reports from inside its cgroup (mocked here; see RealScope for the kernel)."""

    def run_worker(self, tmp, compiler, controller='default', code=0, write=True, flock=True, timed_out=False):
        a = argparse.Namespace(id='hub', out=str(Path(tmp) / 'o'), cap_bytes=CAP, cmd=CMD, copy=None, timeout=3000,
                               protect=['%s=%s' % (n, __file__) for n in lm.PROTECTED])
        def inner(cmd, timeout):
            if write: (Path(tmp) / 'o' / 'hub.js').write_text('hi')
            return code, timed_out
        ctl = enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=1000) if controller == 'default' else controller
        reads = [compiler, compiler]  # before, after
        def read(cg, overridden):
            return ctl if cg == 'CTL' else (reads.pop(0) if reads else compiler)
        with patch.object(lm, 'run_inner', inner), patch.object(lm, 'cgroup_dir', return_value=('CG', False)), \
                patch.object(lm, 'controller_cgroup', return_value='CTL' if controller is not None else None), \
                patch.object(lm, 'read_enforcement', read), \
                patch.object(lm, 'source_identity', return_value=SRC), patch.object(lm, 'tool_identity', return_value=TOOLS), \
                patch.object(lm, 'native_flock', return_value=(flock, lm.REAL_LOCK)):
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

    def test_compiler_without_kernel_cap_never_runs_and_fails(self):
        for e in (enforcement(memory_max='max'), enforcement(memory_swap_max='max'), enforcement(memory_max=lm.CONTROLLER_CAP)):
            with tempfile.TemporaryDirectory() as t:
                rc, r = self.run_worker(t, e)
                self.assertEqual(rc, 1, e); self.assertIsNone(r['exit']); self.assertNotIn('command', r['phases'])

    def test_after_the_fact_violations_fail(self):
        for e in (enforcement(oom_kill=1), enforcement(memory_peak=CAP + 1), enforcement(swap_peak=4096), enforcement(memory_peak=None)):
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

    def test_unenforced_or_missing_controller_fails(self):
        for c in (enforcement(memory_max='max'), None):
            with tempfile.TemporaryDirectory() as t:
                self.assertEqual(self.run_worker(t, enforcement(), controller=c)[0], 1)


class Contract(unittest.TestCase):
    """The exact command shapes lane 3's heavy-guard profiles (PR #854) admit."""

    def test_sum_of_caps_and_decimal_shard_cap(self):
        self.assertLessEqual(lm.COMPILER_CAP, 3_700_000_000)
        self.assertLess(lm.COMPILER_CAP + lm.CONTROLLER_CAP, 4 * lm.GIB)

    def test_profiles(self):
        self.assertEqual(lm.PROFILES, {'proof': 'proof-shard', 'hub': 'compile-shard', 'web': 'compile-shard', 'mobile': 'compile-shard'})

    def test_compile_inner_commands_are_exactly_the_allowed_ones(self):
        self.assertEqual({k: tuple(v[0]) for k, v in lm.FIXED.items()},
                         {'hub': ('bend', 'src/server/main.bend', '-o', '{out}/hub.js'),
                          'web': ('bend', 'src/web/index.html', '-o', '{out}/web'),
                          'mobile': ('bend', 'src/mobile/index.html', '-o', '{out}/mobile-web')})

    def test_proof_job_is_structured_not_sh_c(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as t:
            d = Path(t); (d / 'manifest.json').write_text(json.dumps({'shards': [{'id': '000'}]}))
            (job,) = lm.proof_jobs(d)
        self.assertEqual(job['cmd'][:3], ['python3', 'scripts/proof-shards.py', 'run'])
        for o in ('--out', '--shard', '--expect', '--result'):
            self.assertEqual(job['cmd'].count(o), 1)
        self.assertTrue(job['pre'] and 'generate' in job['pre'][0])  # generate runs in the launcher, not under the guard

    def test_boat_command_nests_controller_launcher_only(self):
        job = {'id': 'hub', 'kind': 'hub', 'cmd': lm.FIXED['hub'][0]}
        argv, rel = lm.boat_argv(job, Path('/x/r1'), 3000, ['bend.ts=/p'])
        cmd = argv[argv.index('--') + 1:]
        self.assertEqual(cmd[:3], ['systemd-run', '--user', '--scope'])
        self.assertIn('MemoryMax=%d' % lm.CONTROLLER_CAP, cmd); self.assertIn('MemorySwapMax=0', cmd)
        self.assertTrue(any(c.startswith('--unit=lowmem-ctl-hub-') for c in cmd))
        self.assertIn('launch', cmd)
        self.assertNotIn('heavy-guard.py', ' '.join(cmd))  # the launcher creates the compiler scope and runs the guard in it
        self.assertNotIn('xlarge', argv); self.assertEqual(argv[argv.index('--size') + 1], 'default')


def real_scope_works():
    try:
        return subprocess.run(['systemd-run', '--user', '--scope', '-q', '-p', 'MemoryMax=64M', '-p', 'MemorySwapMax=0', 'true'],
                              capture_output=True, timeout=20).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


LM = str(ROOT / 'scripts/lowmem-build.py')
LOAD = "import importlib.util as u;s=u.spec_from_file_location('lm',%r);lm=u.module_from_spec(s);s.loader.exec_module(lm);" % LM


@unittest.skipUnless(real_scope_works(), 'needs a systemd user scope with the memory controller')
class RealScope(unittest.TestCase):
    """Negative controls against the actual kernel. A private lock file stands in for the shared bend lock: this shows the
    process hierarchy and descriptor inheritance, NOT the guard-to-worker chain on /tmp/bp-bend.lock."""
    SMALL = 64 * 1024 * 1024

    def chain(self, inner_code, fd=None):
        """controller scope (lowmem-ctl-*) -> compiler scope -> inner_code: the hierarchy `launch` builds."""
        unit = 'lowmem-ctl-test-%d-%d' % (os.getpid(), time.monotonic_ns())
        outer = (LOAD + "import subprocess,sys;r=subprocess.run(lm.systemd_scope(%d,['--',sys.executable,'-c',%r]),pass_fds=%r);sys.exit(r.returncode)"
                 % (lm.COMPILER_CAP, LOAD + inner_code, [fd] if fd is not None else []))
        argv = lm.systemd_scope(lm.CONTROLLER_CAP, ['--unit=' + unit, '--', sys.executable, '-c', outer])
        return subprocess.run(argv, capture_output=True, text=True, pass_fds=[fd] if fd is not None else [], timeout=120)

    def test_hierarchy_limits_are_truthful_and_sum_under_4gib(self):
        inner = ("import json,sys;cg,ov=lm.cgroup_dir();ctl=lm.controller_cgroup();"
                 "print(json.dumps({'own':lm.read_enforcement(cg,ov),'ctl':ctl and lm.read_enforcement(ctl,False),"
                 "'child':lm.run_inner([sys.executable,'-c','pass'],20)}))")
        p = self.chain(inner)
        self.assertEqual(p.returncode, 0, p.stderr)
        data = json.loads(p.stdout.strip().splitlines()[-1])
        own, ctl = data['own'], data['ctl']
        self.assertEqual((own['memory_max'], own['memory_swap_max']), (lm.COMPILER_CAP, 0))
        self.assertEqual((ctl['memory_max'], ctl['memory_swap_max']), (lm.CONTROLLER_CAP, 0))
        self.assertNotEqual(own['cgroup'], ctl['cgroup'])
        self.assertLess(own['memory_max'] + ctl['memory_max'], 4 * lm.GIB)
        self.assertEqual(lm.enforcement_problems(own, lm.COMPILER_CAP), [])
        self.assertEqual(data['child'], [0, False])

    def test_controller_not_discoverable_outside_a_lowmem_scope(self):
        p = subprocess.run([sys.executable, '-c', LOAD + "print(lm.controller_cgroup())"], capture_output=True, text=True)
        self.assertEqual(p.stdout.strip(), 'None')

    def test_hog_in_the_compiler_scope_is_killed_by_the_kernel(self):
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'finished'
            hog = "b=bytearray(400*1024*1024)\nfor i in range(0,len(b),4096): b[i]=1\nopen(%r,'w')" % str(marker)
            p = subprocess.run(lm.systemd_scope(self.SMALL, ['--', sys.executable, '-c', hog]), capture_output=True, timeout=120)
            self.assertNotEqual(p.returncode, 0); self.assertFalse(marker.exists())

    def test_run_inner_timeout_kills(self):
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'late'
            self.assertEqual(lm.run_inner(['sh', '-c', 'sleep 5; touch %s' % marker], 0.3), (None, True))
            time.sleep(0.3); self.assertFalse(marker.exists())

    def test_lock_descriptor_survives_the_chain_and_run_inner(self):
        import fcntl
        with tempfile.TemporaryDirectory() as t:
            lock, sink = Path(t) / 'lock', Path(t) / 'ino.txt'
            fd = os.open(lock, os.O_CREAT | os.O_RDWR); self.addCleanup(os.close, fd)
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB); os.set_inheritable(fd, True)
            child = "import os;open(%r,'w').write(str(os.fstat(int(os.environ['BACKPLANE_HEAVY_LOCK_FD'])).st_ino))" % str(sink)
            inner = "import sys;print(lm.run_inner([sys.executable,'-c',%r],20))" % child
            with patch.dict(os.environ, {'BACKPLANE_HEAVY_LOCK_FD': str(fd)}):
                p = self.chain(inner, fd)
            self.assertEqual(p.returncode, 0, p.stderr)
            self.assertEqual(sink.read_text(), str(os.fstat(fd).st_ino))

    def test_unadmitted_worker_runs_nothing(self):
        with tempfile.TemporaryDirectory() as t:
            marker = Path(t) / 'ran'
            env = {k: v for k, v in os.environ.items() if k != 'BACKPLANE_HEAVY_LOCK_FD'}
            p = subprocess.run([sys.executable, LM, 'worker', '--id', 'hub', '--out', str(Path(t) / 'o'),
                                '--timeout', '60', '--', 'touch', str(marker)], capture_output=True, text=True, env=env, cwd=ROOT)
            r = json.loads((Path(t) / 'o/hub.receipt.json').read_text())
            self.assertEqual(p.returncode, 1); self.assertFalse(marker.exists())
            self.assertIsNone(r['exit']); self.assertNotIn('command', r['phases']); self.assertIsNone(r['controller'])


class MergedGuard(unittest.TestCase):
    """The worker argv this harness builds is accepted by the repository's own heavy-guard (merged PR #854)."""

    def setUp(self):
        spec = importlib.util.spec_from_file_location('guard', ROOT / 'scripts/heavy-guard.py')
        self.g = importlib.util.module_from_spec(spec); spec.loader.exec_module(self.g)
        self.py = str(Path(sys.executable).resolve())

    def worker(self, job, out='build/lowmem/r/x'):
        return [self.py, str(ROOT / 'scripts/lowmem-build.py'), 'worker', '--id', job['id'], '--out', out, '--cap-bytes', str(CAP),
                '--timeout', '3000', '--protect', 'bend.ts=/p', *(['--copy', job['copy']] if job.get('copy') else []), '--', *job['cmd']]

    def test_compile_shapes_accepted_and_others_refused(self):
        for kind in ('hub', 'web', 'mobile'):
            job = {'id': kind, 'cmd': lm.FIXED[kind][0]}
            self.assertTrue(self.g.worker_contract('compile-shard', self.worker(job)), kind)
        bad = {'id': 'hub', 'cmd': ['bend', 'src/server/main.bend', '-o', '/tmp/evil.js']}
        self.assertFalse(self.g.worker_contract('compile-shard', self.worker(bad)))
        self.assertFalse(self.g.worker_contract('compile-shard', self.worker({'id': 'h', 'cmd': ['sh', '-c', 'bend PROOF.bend']})))

    def test_proof_shape_accepted_and_sh_c_refused(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as t:
            d = Path(t); (d / 'manifest.json').write_text(json.dumps({'shards': [{'id': '000'}]}))
            (job,) = lm.proof_jobs(d)
        self.assertTrue(self.g.worker_contract('proof-shard', self.worker(job)))
        self.assertFalse(self.g.worker_contract('proof-shard', self.worker({**job, 'cmd': ['sh', '-c', 'x']})))

    def test_cap_over_shard_cap_refused(self):
        w = self.worker({'id': 'hub', 'cmd': lm.FIXED['hub'][0]})
        w[w.index('--cap-bytes') + 1] = str(self.g.SHARD_CAP + 1)
        self.assertFalse(self.g.worker_contract('compile-shard', w))


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

    def test_reference_without_mobile_refuses_before_any_job(self):
        with tempfile.TemporaryDirectory() as t:
            t = Path(t)
            src = lm.source_identity()
            if not src['clean']:
                self.skipTest('needs a clean tree')
            q = {**QUAL, 'source': {k: src[k] for k in ('head', 'tree', 'closure_sha256')}, 'clean': True}
            q['source']['clean'] = True
            ref = {'schema': 2, 'source': {'head': src['head'], 'tree': src['tree']}, 'producer': {'x': 1},
                   'outputs': {'hub': {'hub.js': H('1')}, 'web': {'web/a.js': H('2')}}}
            (t / 'q.json').write_text(json.dumps(q)); (t / 'r.json').write_text(json.dumps(ref))
            p = self.cli('run', '--jobs', 'hub,web,mobile', '--workdir', str(t / 'w'), '--qualification', str(t / 'q.json'),
                         '--reference', str(t / 'r.json'))
            self.assertEqual(p.returncode, 20)
            self.assertIn('no output for: mobile', p.stderr)

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
