import json, os, subprocess, sys, tempfile, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import importlib.util
spec = importlib.util.spec_from_file_location('lowmem', ROOT / 'scripts' / 'lowmem-build.py')
lm = importlib.util.module_from_spec(spec); spec.loader.exec_module(lm)

import argparse
from unittest.mock import patch

SRC = {'head': 'a' * 40, 'tree': 'b' * 40, 'clean': True}
TOOLS = {'bend': {'version': '1', 'sha256': 'c' * 64}, 'bun': None}
CAP = lm.CAP_BYTES


def enforcement(**kw):
    e = {'executor': 'cgroup-v2', 'cgroup': '/x', 'overridden': False, 'memory_max': CAP, 'memory_swap_max': 0,
         'memory_peak': 2 * lm.GIB, 'swap_peak': 0, 'oom': 0, 'oom_kill': 0, 'oom_group_kill': 0}
    e.update(kw)
    return e


def receipt(**kw):
    r = {'schema': 2, 'id': 'hub', 'exit': 0, 'cap_bytes': CAP, 'peak_bytes': 2 * lm.GIB, 'enforcement': enforcement(), 'controller': enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=100 * 1024 ** 2),
         'swap_activity': False, 'boat_sandbox': True, 'mem_total_bytes': 8 * lm.GIB, 'source': SRC, 'tools': TOOLS,
         'outputs': {'hub.js': 'x'}}
    r.update(kw)
    return r


JOB = {'id': 'hub', 'kind': 'hub'}
REF = {'outputs': {'hub': {'hub.js': 'x'}}}


class Judge(unittest.TestCase):
    def j(self, r, ref=REF, src=SRC, tools=TOOLS):
        return lm.judge(JOB, r, ref, src, tools, CAP)

    def test_pass(self):
        self.assertEqual(self.j(receipt()), [])

    def test_kernel_cap_required_exactly(self):
        for e in (enforcement(memory_max='max'), enforcement(memory_max=None), enforcement(memory_max=CAP + 4096),
                  enforcement(memory_swap_max='max'), enforcement(memory_swap_max=1), enforcement(memory_peak=None),
                  enforcement(memory_peak=CAP + 1), enforcement(oom_kill=1), enforcement(oom=1), enforcement(overridden=True),
                  enforcement(executor='rss-poller'), enforcement(swap_peak=4096), None):
            self.assertTrue(self.j(receipt(enforcement=e)), e)

    def test_not_a_normal_boat_job(self):
        self.assertTrue(self.j(receipt(boat_sandbox=False)))
        self.assertTrue(self.j(receipt(mem_total_bytes=64 * lm.GIB)))
        self.assertTrue(self.j(receipt(schema=1)))

    def test_controller_cap_enforced_and_sum_under_4gib(self):
        self.assertEqual(lm.COMPILER_CAP, 3699998720); self.assertEqual(lm.CONTROLLER_CAP, 536870912)
        self.assertLess(lm.COMPILER_CAP + lm.CONTROLLER_CAP, 4 * lm.GIB)
        for c in (None, enforcement(memory_max='max'), enforcement(memory_max=lm.CONTROLLER_CAP, memory_swap_max=1),
                  enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=lm.CONTROLLER_CAP + 1)):
            self.assertTrue(self.j(receipt(controller=c)), c)

    def test_exit_75_is_a_reported_refusal(self):
        self.assertTrue(self.j(receipt(exit=75)))

    def test_cap_must_be_under_4gib(self):
        self.assertTrue(lm.receipt_problems(receipt(cap_bytes=4 * lm.GIB), 4 * lm.GIB))

    def test_exit_swap_missing(self):
        self.assertTrue(self.j(receipt(exit=1)))
        self.assertTrue(self.j(receipt(exit=None)))
        self.assertTrue(any('SWAP' in w for w in self.j(receipt(swap_activity=True))))
        self.assertTrue(self.j(None))

    def test_bytes_differ_or_missing_reference(self):
        self.assertTrue(any('BYTE-IDENTICAL' in w for w in self.j(receipt(outputs={'hub.js': 'y'}))))
        self.assertTrue(any('no reference' in w for w in self.j(receipt(), ref={'outputs': {}})))

    def test_provenance_bound_to_qualified_compiler(self):
        self.assertTrue(self.j(receipt(source={**SRC, 'tree': 'd' * 40})))
        self.assertTrue(self.j(receipt(tools={'bend': {'version': '2', 'sha256': 'c' * 64}, 'bun': None})))
        self.assertTrue(self.j(receipt(), tools=None))
        dirty = {**SRC, 'clean': False}
        self.assertTrue(self.j(receipt(source=dirty), src=dirty))


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
        self.assertTrue(lm.judge(job, r, REF, SRC, TOOLS, CAP))
        self.assertEqual(lm.judge(job, r, REF, SRC, TOOLS, CAP, proof=[]), [])


class Worker(unittest.TestCase):
    """The worker only trusts limits the kernel reports from inside the cgroup (mocked here; no cgroup is created)."""

    def run_worker(self, tmp, compiler, controller=None, code=0, write=True):
        a = argparse.Namespace(id='hub', out=str(Path(tmp) / 'o'), cap_bytes=CAP, cmd=['true'], copy=None, no_scope=True)
        def capped(cmd, cap, report):
            if write: (Path(tmp) / 'o' / 'hub.js').write_text('hi')
            return code, compiler
        ctl = controller or enforcement(memory_max=lm.CONTROLLER_CAP, memory_peak=1000)
        with patch.object(lm, 'run_capped', capped), patch.object(lm, 'cgroup_dir', return_value=(Path(tmp), False)), \
                patch.object(lm, 'read_enforcement', return_value=ctl), patch.object(lm, 'source_identity', return_value=SRC), \
                patch.object(lm, 'tool_identity', return_value=TOOLS):
            rc = lm.worker(a)
        return rc, json.loads((Path(a.out) / 'hub.receipt.json').read_text())

    def test_enforced_cgroups_recorded(self):
        with tempfile.TemporaryDirectory() as t:
            rc, r = self.run_worker(t, enforcement(memory_peak=123456789))
            self.assertEqual(rc, 0)
            self.assertEqual(r['peak_bytes'], 123456789)
            self.assertEqual(r['outputs'], {'hub.js': lm.sha(b'hi')})
            self.assertEqual(r['controller']['memory_max'], lm.CONTROLLER_CAP)

    def test_compiler_without_kernel_cap_fails(self):
        for e in (None, enforcement(memory_max='max'), enforcement(memory_swap_max='max'), enforcement(oom_kill=1),
                  enforcement(memory_peak=CAP + 1)):
            with tempfile.TemporaryDirectory() as t:
                self.assertEqual(self.run_worker(t, e)[0], 1, e)

    def test_nonzero_or_no_output_fails(self):
        with tempfile.TemporaryDirectory() as t:
            self.assertEqual(self.run_worker(t, enforcement(), code=1)[0], 1)
        with tempfile.TemporaryDirectory() as t:
            self.assertEqual(self.run_worker(t, enforcement(), write=False)[0], 1)

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
            q.write_text(json.dumps({'qualified': True, 'source': {'tree': '0' * 40}, 'tools': TOOLS}))
            p = self.cli('run', '--jobs', 'hub', '--workdir', t, '--runner', 'local', '--qualification', str(q))
            self.assertEqual(p.returncode, 20)
            self.assertIn('another source tree', p.stderr)

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
