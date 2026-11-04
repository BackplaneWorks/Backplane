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

    def test_abi_edges_validate_after_combining_service(self):
        """Regression: runtime-only has no executable for CLOSURE.from edges."""
        import json
        import platform
        from unittest.mock import patch
        def require(ok, msg):
            if not ok:
                raise ValueError(msg)
        with tempfile.TemporaryDirectory() as t:
            root = Path(t)
            build = root / 'new'; (build / 'logs').mkdir(parents=True)
            (build / 'backplane-serve').write_text('new-binary')
            previous = root / 'previous/bootstrap-service'
            runtime = previous / 'runtime'; runtime.mkdir(parents=True)
            (runtime / 'lib').write_text('retained-library')
            log = root / 'version.log'; log.write_text('backplane 0.14.0\n')
            data = {'BUILD.json': {'source_commit': 'new', 'source_tree': 'newtree',
                    'target': 'linux-x64', 'service': 'backplane-serve', 'binary_sha256': 'newhash'},
                'relink-provenance.json': {'previousSourceCommit': 'old', 'previousBinarySha256': 'oldhash',
                    'originalBuild': {'source_tree': 'oldtree'}, 'priorRuntimeQualificationSha256': 'oldqual'},
                'RUNTIME-QUALIFICATION.json': {'status': 'PASS', 'target': 'linux-x64',
                    'sourceCommit': 'old', 'sourceTree': 'oldtree', 'binarySha256': 'oldhash',
                    'members': {'lib': {'type': 'file', 'mode': 0o644}}}}
            calls = []
            def closure(folder, target):
                require((folder / 'backplane-serve').is_file(), 'Invalid measured edge')
                calls.append(folder)
            def combine(request, base, out):
                out.mkdir(); payload = out / 'payload'; payload.mkdir()
                (payload / 'backplane-serve').write_text('new-binary')
                closure(payload, request['target'])
                return {'status': 'combined'}
            producer = SimpleNamespace(VERSION='0.14.0', require=require,
                read=lambda p: data[p.name],
                digest=lambda p: 'newhash' if p.name == 'backplane-serve' else 'oldqual',
                inventory=lambda p: data['RUNTIME-QUALIFICATION.json']['members'],
                relative=lambda p: p,
                reference=lambda p: {'path': str(p)},
                write=lambda p, v: p.write_text(json.dumps(v)),
                runtime_closure=closure, component=combine)
            args = SimpleNamespace(target='linux-x64', build=build, previous=root / 'previous',
                source='new', launch_log=log, out=root / 'assembled/component')
            with patch.object(platform, 'system', return_value='Linux'), \
                    patch.object(platform, 'machine', return_value='x86_64'):
                relink.component(args, producer, None)
            self.assertEqual(calls, [args.out / 'payload'])


if __name__ == '__main__':
    unittest.main()
