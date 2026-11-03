import copy, hashlib, importlib.util, json, pathlib, tempfile, unittest

w = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('release', w / 'scripts/service-release.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
low = m.lowmem_module()

COMMIT, TREE, BEND = 'a' * 40, 'b' * 40, 'c' * 64
HUB = hashlib.sha256(b'raw emitted JS').hexdigest()
CAP = low.COMPILER_CAP


def enf(cap, peak):
    return {'executor': 'cgroup-v2', 'cgroup': '/x', 'overridden': False, 'memory_max': cap, 'memory_swap_max': 0,
            'memory_peak': peak, 'swap_peak': 0, 'oom': 0, 'oom_kill': 0, 'oom_group_kill': 0}


class Handoff(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = pathlib.Path(self.tmp.name); (self.root / 'receipts').mkdir()
        self.release = {'commit': COMMIT, 'tree': TREE}
        self.compiler = {'sha256': BEND, 'version': '2.0.36'}
        self.pre = (self.root, {'files': {'server.js': {'sha256': HUB}}})
        self.source = {'head': COMMIT, 'tree': TREE, 'clean': True}
        self.tools = {'bend': {'version': 'v', 'sha256': BEND}, 'bun': None}
        self.report = {'schema': 2, 'kind': 'backplane-lowmem-build-report', 'state': 'passed', 'release': True,
                       'complete': True, 'qualified': True, 'fallback': False, 'runner': 'boat', 'cap_bytes': CAP,
                       'source': self.source, 'tools': self.tools, 'jobs': {},
                       'qualification': {'sha256': '1' * 64}, 'reference': {'sha256': '2' * 64}}
        for name, out in (('hub', {'hub.js': HUB}), ('web', {'index.html': '3' * 64}), ('mobile', {'index.html': '4' * 64})):
            self.job(name, out)

    def job(self, name, out, **patch):
        r = {'schema': 2, 'id': name, 'exit': 0, 'cap_bytes': CAP, 'enforcement': enf(CAP, 10 ** 9),
             'controller': enf(low.CONTROLLER_CAP, 10 ** 8), 'swap_activity': False, 'boat_sandbox': True,
             'mem_total_bytes': 8 * low.GIB, 'source': self.source, 'tools': self.tools, 'outputs': out}
        r.update(patch)
        path = self.root / 'receipts' / (name + '.receipt.json')
        path.write_text(json.dumps(r))
        self.report['jobs'][name] = {'ok': True, 'why': [], 'kind': name, 'outputs': out,
                                     'receipt': m.local_reference(path, self.root)}

    def check(self):
        (self.root / 'report.json').write_text(json.dumps(self.report))
        return m.verified_lowmemory(m.local_reference(self.root / 'report.json', self.root), self.root, self.release,
                                    self.compiler, self.pre, low)

    def reject(self):
        with self.assertRaises((ValueError, OSError, KeyError)):
            self.check()

    def test_valid(self):
        self.assertEqual(self.check(), (self.root / 'report.json').resolve())

    def test_requires_verified_preemission(self):
        self.pre = None; self.reject()

    def test_report_fields(self):
        for k, v in (('state', 'failed'), ('release', False), ('complete', False), ('qualified', False), ('fallback', True),
                     ('runner', 'local'), ('schema', 1), ('cap_bytes', CAP + 4096)):
            self.report[k] = v; self.reject(); self.setUp()

    def test_source_and_compiler_binding(self):
        self.report['source'] = {**self.source, 'tree': 'd' * 40}; self.reject(); self.setUp()
        self.report['tools'] = {'bend': {'sha256': 'e' * 64}, 'bun': None}; self.reject()

    def test_subset_or_empty(self):
        for drop in ('web', 'mobile', 'hub'):
            del self.report['jobs'][drop]; self.reject(); self.setUp()
        self.report['jobs'] = {}; self.reject()

    def test_hub_bytes_must_equal_admitted_emission(self):
        self.job('hub', {'hub.js': 'f' * 64}); self.reject(); self.setUp()
        self.job('hub', {'hub.js': HUB, 'extra.js': HUB}); self.reject()

    def test_receipt_tamper_and_overcap_and_swap(self):
        (self.root / 'receipts/web.receipt.json').write_text('{}'); self.reject(); self.setUp()
        self.job('web', {'index.html': '3' * 64}, enforcement=enf(CAP, CAP + 1)); self.reject(); self.setUp()
        self.job('web', {'index.html': '3' * 64}, swap_activity=True); self.reject(); self.setUp()
        self.job('web', {'index.html': '3' * 64}, boat_sandbox=False); self.reject(); self.setUp()
        self.job('web', {'index.html': '3' * 64}, controller=enf(low.CONTROLLER_CAP, 10 ** 9 * 9)); self.reject()

    def test_unbound_receipt_path(self):
        self.report['jobs']['web']['receipt']['path'] = '../x'; self.reject()

    def test_receipt_differs_from_report(self):
        self.report['jobs']['web']['outputs'] = {'index.html': '9' * 64}; self.reject()

    def test_missing_bindings(self):
        del self.report['qualification']; self.reject()


if __name__ == '__main__':
    unittest.main()
