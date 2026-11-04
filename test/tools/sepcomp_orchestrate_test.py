import importlib.util, json, os, random, shutil, subprocess, sys, tempfile, threading, time, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('sc', ROOT / 'scripts/sepcomp-orchestrate.py')
sc = importlib.util.module_from_spec(spec); spec.loader.exec_module(sc)
TOOLS = sc.FAKE_TOOLS


def fixture(t):
    """server -> a,b -> c ; a<->b cycle? no: SCC is x<->y (atomic), plus an effect companion and a header include."""
    r = Path(t)
    files = {
        'src/server/main.bend': 'import ./../lib/a.bend\nimport ./../lib/x.bend\nimport Base\n# main\n',
        'src/web/app.bend': 'import ./../lib/a.bend\n',
        'src/mobile/app.bend': 'import ./../lib/c.bend\n',
        'src/lib/a.bend': 'import ./c.bend\nimport "./fx.c"\n# a\n',
        'src/lib/c.bend': '# c leaf\n',
        'src/lib/x.bend': 'import ./y.bend\nimport ./c.bend\n# x\n',
        'src/lib/y.bend': 'import ./x.bend\n# y (cycle with x)\n',
        'src/lib/fx.c': '#include "fx.h"\nint fx;\n',
        'src/lib/fx.h': '#define FX 1\n',
    }
    for f, text in files.items():
        (r / f).parent.mkdir(parents=True, exist_ok=True); (r / f).write_text(text)
    return r


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.repo = fixture(Path(self.tmp.name) / 'repo' if (Path(self.tmp.name) / 'repo').mkdir() is None else None)
        self.plan = sc.make_plan(self.repo, TOOLS)

    def run_fake(self, name='run', worker=None, parallel=3, mutate=None):
        out = Path(self.tmp.name) / name
        res = sc.schedule(self.plan, worker or sc.FakeWorker(), out, parallel)
        if res['passed']:
            ordered = {u: sc.sha((out / 'units' / u / 'object.bin').read_bytes()) for u in self.plan['order']}
            sc.FakeLinker()(self.plan, ordered, out / 'link')
        (out / 'plan.json').write_text(json.dumps(self.plan, sort_keys=True))
        if mutate:
            mutate(out)
        return out, res

    def verify(self, out, allow_fake=True, repo=None):
        return sc.verify_run(repo or self.repo, out, TOOLS, out / 'plan.json', allow_fake=allow_fake)

    def edit(self, out, rel, fn):
        p = out / rel; d = json.loads(p.read_text()); fn(d); p.write_text(json.dumps(d, sort_keys=True))

    def unit_of(self, member):
        return next(u for u, v in self.plan['units'].items() if member in v['members'])


class Inventory(Base):
    def test_scc_is_atomic_and_inventory_complete(self):
        units = self.plan['units']
        cyc = [v for v in units.values() if len(v['members']) > 1]
        self.assertEqual(len(cyc), 1); self.assertEqual(cyc[0]['members'], ['src/lib/x.bend', 'src/lib/y.bend'])
        self.assertEqual(sum(len(v['members']) for v in units.values()), 7)  # every module exactly once
        self.assertEqual(len(units), 6)
        a = units[self.unit_of('src/lib/a.bend')]
        self.assertEqual(sorted(a['sources']), ['src/lib/a.bend', 'src/lib/fx.c', 'src/lib/fx.h'])  # companion + transitive header

    def test_dependencies_and_canonical_order(self):
        order, units = self.plan['order'], self.plan['units']
        for u in order:
            for d in units[u]['deps']:
                self.assertLess(order.index(d), order.index(u))
        self.assertEqual(self.plan, sc.make_plan(self.repo, TOOLS))   # deterministic

    def test_plan_changes_with_any_source_tool_or_option(self):
        base = self.plan['plan_sha256']
        for f in ('src/lib/c.bend', 'src/lib/fx.h', 'src/lib/y.bend'):
            p = self.repo / f; old = p.read_text(); p.write_text(old + '# changed\n')
            self.assertNotEqual(sc.make_plan(self.repo, TOOLS)['plan_sha256'], base, f); p.write_text(old)
        for k, v in (('compiler_sha256', '1' * 64), ('base_sha256', '2' * 64), ('options', ['--x']), ('env', {})):
            self.assertNotEqual(sc.make_plan(self.repo, {**TOOLS, k: v})['plan_sha256'], base, k)

    def test_unparseable_or_missing_input_fails_closed(self):
        (self.repo / 'src/lib/c.bend').write_text('import \x01weird\n')
        with self.assertRaises(sc.PlanError): sc.make_plan(self.repo, TOOLS)
        (self.repo / 'src/lib/c.bend').write_text('import ./nope.bend\n')
        with self.assertRaises(sc.PlanError): sc.make_plan(self.repo, TOOLS)

    def test_matches_bend_closure_on_the_real_repo(self):
        modules, comps = sc.inventory(ROOT)
        want = subprocess.check_output([sys.executable, str(ROOT / 'scripts/bend-closure.py'), *sc.ENTRY_ROOTS], cwd=ROOT, text=True).splitlines()
        got = sorted(set(modules) | {c for v in comps.values() for c in v})
        self.assertEqual(got, sorted(want))

    def test_real_repo_plan_shape(self):
        plan = sc.make_plan(ROOT, TOOLS)
        modules = {m for v in plan['units'].values() for m in v['members']}
        self.assertEqual(len(modules), sum(len(v['members']) for v in plan['units'].values()))  # no module twice
        self.assertEqual(plan['order'], sc.topo_order(plan['units']))


class Schedule(Base):
    def test_good_run_verifies_and_is_clearly_fake(self):
        out, res = self.run_fake()
        self.assertTrue(res['passed']); self.assertEqual(self.verify(out), [])
        self.assertTrue(self.verify(out, allow_fake=False))   # FAKE evidence is refused where a real worker is required

    def test_no_wave_barriers_dependency_ready(self):
        """a slow unit on one branch must not hold back independent units on another."""
        slow = self.unit_of('src/lib/c.bend')
        starts, lock = {}, threading.Lock()
        w = sc.FakeWorker(delay=lambda u: 0.4 if u == slow else 0.0)
        orig = w.__call__
        def timed(plan, unit, deps, out):
            with lock: starts[unit['unit']] = time.monotonic()
            return orig(plan, unit, deps, out)
        out, res = self.run_fake(worker=timed, parallel=4)
        self.assertTrue(res['passed'])
        indep = [u for u in self.plan['units'] if slow not in self.plan['units'][u]['deps'] and u != slow
                 and not self.plan['units'][u]['deps']]
        self.assertTrue(all(starts[u] < starts[slow] + 0.3 for u in indep))

    def test_records_do_not_depend_on_completion_order(self):
        digests = set()
        for seed, par in ((1, 1), (2, 2), (3, 8), (4, 3)):
            rnd = random.Random(seed)
            out, res = self.run_fake(name='r%d' % seed, worker=sc.FakeWorker(delay=lambda u: rnd.random() * 0.02), parallel=par)
            self.assertEqual(self.verify(out), [])
            digests.add(json.dumps({u: json.loads((out / 'units' / u / 'receipt.json').read_text()) for u in self.plan['order']}, sort_keys=True)
                        + (out / 'link/program.bin').read_text())
        self.assertEqual(len(digests), 1)

    def test_failure_stops_new_work_and_never_passes(self):
        bad = self.unit_of('src/lib/c.bend')
        out, res = self.run_fake(worker=sc.FakeWorker(faults={bad: 'fail'}))
        self.assertFalse(res['passed']); self.assertIn(bad, res['failed'])
        consumers = [u for u, v in self.plan['units'].items() if bad in v['deps']]
        self.assertTrue(all(c in res['not_run'] for c in consumers))   # a failed dependency fails every consumer

    def test_admission_refusal_is_retried_then_fails_closed(self):
        u = self.unit_of('src/lib/c.bend')
        calls = {'n': 0}
        class Flaky(sc.FakeWorker):
            def __call__(s, plan, unit, deps, out):
                if unit['unit'] == u and calls['n'] < 2:
                    calls['n'] += 1; raise sc.Deferred('window')
                return super().__call__(plan, unit, deps, out)
        out, res = self.run_fake(worker=Flaky())
        self.assertTrue(res['passed']); self.assertEqual(res['deferrals'][u], 2)
        out, res = sc.schedule(self.plan, sc.FakeWorker(faults={u: 'defer'}), Path(self.tmp.name) / 'never', 2, max_deferrals=2), None
        self.assertFalse(out['passed']); self.assertIn(u, out['failed'])


class Controls(Base):
    """Every control must make verify_run refuse a run the scheduler considered finished."""

    def refuse(self, mutate, needle, worker=None):
        out, res = self.run_fake(worker=worker, mutate=mutate)
        why = self.verify(out)
        self.assertTrue(why, 'accepted a bad run')
        self.assertTrue(any(needle.lower() in w.lower() for w in why), (needle, why))

    def test_forged_artifact_hash(self):
        u = self.unit_of('src/lib/c.bend')
        self.refuse(None, 'does not match the bytes', sc.FakeWorker(faults={u: 'forge-hash'}))

    def test_false_pass_verdict_rejected_but_receipt_passed(self):
        u = self.unit_of('src/lib/c.bend')
        self.refuse(None, 'false-pass', sc.FakeWorker(faults={u: 'false-pass'}))

    def test_stale_dependency_interface(self):
        u = self.unit_of('src/lib/a.bend')
        self.refuse(None, 'stale/forged chain', sc.FakeWorker(faults={u: 'wrong-deps'}))

    def test_interface_changed_after_consumer_built(self):
        c = self.unit_of('src/lib/c.bend')
        def mutate(out):
            (out / 'units' / c / 'interface.bin').write_text('0' * 64)
            self.edit(out, 'units/%s/receipt.json' % c, lambda d: d['artifacts']['interface.bin'].update(sha256=sc.sha('0' * 64)))
        self.refuse(mutate, 'stale/forged chain')

    def test_receipt_status_and_exit_lies(self):
        c = self.unit_of('src/lib/c.bend')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d.update(status='failed')), 'did not pass')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d.update(exit=1)), 'did not pass')

    def test_extra_unknown_key_and_extra_file(self):
        u = self.unit_of('src/lib/c.bend')
        self.refuse(None, 'keys differ', sc.FakeWorker(faults={u: 'extra-key'}))
        self.refuse(lambda out: (out / 'units' / u / 'stray.bin').write_text('x'), 'extra artifacts')
        self.refuse(lambda out: (out / 'units' / 'ffffffffffffffff').mkdir() or (out / 'units/ffffffffffffffff/receipt.json').write_text('{}'), 'extra artifacts')

    def test_missing_artifact_or_receipt(self):
        u = self.unit_of('src/lib/c.bend')
        self.refuse(lambda out: (out / 'units' / u / 'object.bin').unlink(), 'missing artifacts')
        self.refuse(lambda out: (out / 'units' / u / 'receipt.json').unlink(), 'missing artifacts')
        self.refuse(lambda out: (out / 'link/link.json').unlink(), 'missing artifacts')

    def test_duplicate_unit_receipt(self):
        u = self.unit_of('src/lib/c.bend')
        self.refuse(lambda out: shutil.copy(out / 'units' / u / 'receipt.json', out / 'units' / u / 'receipt (copy).json'), 'extra artifacts')

    def test_post_plan_source_mutation(self):
        out, _ = self.run_fake()
        (self.repo / 'src/lib/c.bend').write_text('# c leaf, changed after the plan was frozen\n')
        why = self.verify(out)
        self.assertTrue(any('POST-PLAN MUTATION' in w for w in why), why)

    def test_post_plan_tool_mutation(self):
        out, _ = self.run_fake()
        why = sc.verify_run(self.repo, out, {**TOOLS, 'compiler_sha256': '0' * 64}, out / 'plan.json', allow_fake=True)
        self.assertTrue(any('POST-PLAN MUTATION' in w for w in why))

    def test_receipt_for_another_plan_or_tools_or_caps(self):
        c = self.unit_of('src/lib/c.bend')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d.update(plan_sha256='0' * 64)), 'another plan')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d['tools'].update(compiler_sha256='0' * 64)), 'tool/options')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d['caps'].update(compiler_cap=8 * sc.GIB)), 'caps differ')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d['caps'].update(swap_max=1)), 'caps differ')

    def test_scc_split_or_members_changed(self):
        x = self.unit_of('src/lib/x.bend')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % x, lambda d: d.update(members=['src/lib/x.bend'])), 'atomicity')

    def test_source_identity_in_receipt(self):
        c = self.unit_of('src/lib/c.bend')
        self.refuse(lambda out: self.edit(out, 'units/%s/receipt.json' % c, lambda d: d['sources'].update({'src/lib/c.bend': '0' * 64})), 'source identity')

    def test_link_inputs_missing_duplicate_reordered_or_forged(self):
        order = self.plan['order']
        self.refuse(lambda out: self.edit(out, 'link/link.json', lambda d: d['inputs'].pop()), 'link: inputs')
        self.refuse(lambda out: self.edit(out, 'link/link.json', lambda d: d['inputs'].append(d['inputs'][0])), 'link: inputs')
        self.refuse(lambda out: self.edit(out, 'link/link.json', lambda d: d['inputs'].reverse()), 'link: inputs')
        self.refuse(lambda out: self.edit(out, 'link/link.json', lambda d: d['inputs'][0].__setitem__(1, '0' * 64)), 'link: inputs')

    def test_link_output_differs_from_reference(self):
        def mutate(out):
            (out / 'link/program.bin').write_text('0' * 64)
            self.edit(out, 'link/link.json', lambda d: d.update(output={'sha256': sc.sha('0' * 64), 'bytes': 64}))
        self.refuse(mutate, 'BYTES DIFFER')
        self.refuse(lambda out: (out / 'link/program.bin').write_text('1' * 64), 'link: output hash')

    def test_link_receipt_for_another_plan_or_tools(self):
        self.refuse(lambda out: self.edit(out, 'link/link.json', lambda d: d.update(plan_sha256='0' * 64)), 'link: another plan')
        self.refuse(lambda out: self.edit(out, 'link/link.json', lambda d: d['tools'].update(compiler_sha256='0' * 64)), 'link: tools')

    def test_swapped_object_between_units(self):
        a, c = self.unit_of('src/lib/a.bend'), self.unit_of('src/lib/c.bend')
        def mutate(out):
            t = (out / 'units' / a / 'object.bin').read_text()
            (out / 'units' / a / 'object.bin').write_text((out / 'units' / c / 'object.bin').read_text())
            (out / 'units' / c / 'object.bin').write_text(t)
        self.refuse(mutate, 'does not match the bytes')


class Cli(unittest.TestCase):
    def cli(self, *a):
        return subprocess.run([sys.executable, str(ROOT / 'scripts/sepcomp-orchestrate.py'), *a], capture_output=True, text=True)

    def test_dry_run_launches_nothing_and_labels_everything(self):
        p = self.cli('dry-run')
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn('PROVISIONAL', p.stdout); self.assertIn('UNMEASURED', p.stdout); self.assertIn('FAKE', p.stdout)

    def test_fake_run_then_verify_cli(self):
        with tempfile.TemporaryDirectory() as t:
            out = Path(t) / 'run'
            p = self.cli('fake-run', '--out', str(out), '--parallel', '6')
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
            self.assertNotEqual(self.cli('verify', '--run', str(out)).returncode, 0)   # FAKE evidence refused by default verify


if __name__ == '__main__':
    unittest.main()
