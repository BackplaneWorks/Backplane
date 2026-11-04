#!/usr/bin/env python3
"""Negative and delegation controls for exact configured-service reuse."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('relink', ROOT / 'scripts/service-relink.py')
relink = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relink)


class RelinkTests(unittest.TestCase):
    def test_transform_only(self):
        self.assertEqual(relink.source_equivalence({'scripts/js-native.py': 'old', 'src/server/a.bend': 'x'},
            {'scripts/js-native.py': 'new', 'src/server/a.bend': 'x'}), ['scripts/js-native.py'])

    def test_bend_delta_rejected(self):
        with self.assertRaises(ValueError):
            relink.source_equivalence({'scripts/js-native.py': 'o', 'src/a.bend': 'x'},
                {'scripts/js-native.py': 'n', 'src/a.bend': 'y'})

    def test_runtime_effect_delta_rejected(self):
        with self.assertRaises(ValueError):
            relink.source_equivalence({'scripts/js-native.py': 'o'},
                {'scripts/js-native.py': 'n', 'src/server/effects/host.js': 'new'})

    def test_toolchain_delta_rejected(self):
        with self.assertRaises(ValueError):
            relink.source_equivalence({'scripts/js-native.py': 'o', 'scripts/build-js.sh': 'x'},
                {'scripts/js-native.py': 'n', 'scripts/build-js.sh': 'y'})

    def test_dependency_removed_rejected(self):
        with self.assertRaises(ValueError):
            relink.source_equivalence({'scripts/js-native.py': 'o', 'tools/browser/a.ts': 'x'},
                {'scripts/js-native.py': 'n'})

    def test_unchanged_transform_rejected(self):
        with self.assertRaises(ValueError):
            relink.source_equivalence({'src/a': 'x'}, {'src/a': 'x'})

    def test_nonhelper_delegates_unchanged(self):
        calls = []
        original = lambda *args: calls.append(args) or 'ran'
        hook = relink.helper_reuse(None, None, None, original)
        argv = ['scripts/build-js.sh', 'src/server/main.bend', '/out/service']
        self.assertEqual(hook(argv, '/log', {}, 123), 'ran')
        self.assertEqual(calls, [(argv, '/log', {}, 123)])

    def test_unknown_helper_contract_rejected(self):
        hook = relink.helper_reuse(None, None, None, lambda *args: self.fail('fallback'))
        with self.assertRaises(ValueError):
            hook(['scripts/build-helpers.sh', '/out', '--extra'], '/log', {})

    def test_helper_mismatch_no_fallback(self):
        def require(ok, msg):
            if not ok:
                raise ValueError(msg)
        producer = SimpleNamespace(require=require, digest=lambda _: 'wrong')
        hook = relink.helper_reuse(producer, Path('/old'), {'helpers': {'helper': 'expected'}},
                                  lambda *args: self.fail('fallback'))
        with tempfile.TemporaryDirectory() as t, self.assertRaises(ValueError):
            hook(['scripts/build-helpers.sh', t], Path(t) / 'log', {})

    def test_exact_helper_copied_with_notice_no_compile(self):
        def require(ok, msg):
            if not ok:
                raise ValueError(msg)
        import hashlib
        producer = SimpleNamespace(require=require,
            digest=lambda p: hashlib.sha256(p.read_bytes()).hexdigest())
        with tempfile.TemporaryDirectory() as t:
            old = Path(t) / 'old'; old.mkdir()
            out = Path(t) / 'out'; out.mkdir()
            (old / 'helper').write_bytes(b'accepted-helper')
            (old / 'licenses/helpers').mkdir(parents=True)
            (old / 'licenses/helpers/LICENSE').write_text('retained-notice')
            hook = relink.helper_reuse(producer, old,
                {'helpers': {'helper': producer.digest(old / 'helper')}},
                lambda *args: self.fail('helper compiled'))
            hook(['scripts/build-helpers.sh', out], out / 'log', {})
            self.assertEqual((out / 'helper').read_bytes(), b'accepted-helper')
            self.assertEqual((out / 'helper').stat().st_mode & 0o777, 0o755)
            self.assertEqual((out / 'licenses/helpers/LICENSE').read_text(), 'retained-notice')


if __name__ == '__main__':
    unittest.main()
