import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('existing_windows', ROOT / 'scripts/windows-existing-desktop-package.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class IdentityTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.commit, self.tree = 'a' * 40, 'b' * 40
        release = {'commit': self.commit, 'tree': self.tree, 'version': '0.14.0', 'channel': 'release', 'tag': 'v0.14.0'}
        self.values = {
            'components/COMPONENT-INPUTS.json': {'release': release, 'target': 'windows-x64'},
            'freeze/FREEZE.json': {**release, 'status': 'FROZEN'},
            'component/SERVICE.json': {'sourceCommit': self.commit, 'sourceTree': self.tree, 'version': '0.14.0', 'channel': 'release', 'target': 'windows-x64', 'status': 'VERIFIED'}}

    def check(self, values=None):
        for name, value in (values or self.values).items():
            p = self.root / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(json.dumps(value), encoding='utf-8')
        return module.identities(self.root, self.commit, self.tree)

    def test_exact_composition(self):
        self.assertEqual(self.check()[0]['target'], 'windows-x64')

    def test_other_service_source_rejected(self):
        values = copy.deepcopy(self.values)
        values['component/SERVICE.json']['sourceCommit'] = 'c' * 40
        with self.assertRaises(AssertionError):
            self.check(values)

    def test_other_freeze_tree_rejected(self):
        values = copy.deepcopy(self.values)
        values['freeze/FREEZE.json']['tree'] = 'c' * 40
        with self.assertRaises(AssertionError):
            self.check(values)

    def test_other_target_rejected(self):
        values = copy.deepcopy(self.values)
        values['components/COMPONENT-INPUTS.json']['target'] = 'linux-x64'
        with self.assertRaises(AssertionError):
            self.check(values)


if __name__ == '__main__':
    unittest.main()
