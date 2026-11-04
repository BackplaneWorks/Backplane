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
    def reviewed_pair(self):
        old = {'src/unchanged.bend': 'same'}
        current = dict(old)
        for name, item in relink.REVIEWED_DELTA.items():
            if item['base'] is not None:
                old[name] = item['base']
            current[name] = item['selected']
        return old, current

    def test_exact_frozen_hotfix_delta(self):
        old, current = self.reviewed_pair()
        self.assertEqual(relink.source_equivalence(old, current), sorted(relink.REVIEWED_DELTA))

    def test_every_reviewed_path_rejects_modified_bytes(self):
        for name in relink.REVIEWED_DELTA:
            with self.subTest(name=name):
                old, current = self.reviewed_pair()
                current[name] = 'unreviewed'
                with self.assertRaises(ValueError):
                    relink.source_equivalence(old, current)

    def test_unrelated_main_or_toolchain_delta_rejected(self):
        for name in ('src/a.bend', 'scripts/build-js.sh', 'tools/browser/a.ts',
                     'src/server/effects/host.js', '.github/unreviewed.yml'):
            with self.subTest(name=name):
                old, current = self.reviewed_pair()
                current[name] = 'unreviewed'
                with self.assertRaises(ValueError):
                    relink.source_equivalence(old, current)

    def test_reviewed_change_cannot_be_omitted(self):
        old, current = self.reviewed_pair()
        current['src/server/effects/storeio.js'] = old['src/server/effects/storeio.js']
        with self.assertRaises(ValueError):
            relink.source_equivalence(old, current)

    def test_old_version_fields_remain_original_and_inputs_immutable(self):
        new = {'version': '0.14.1', 'versionStamp': '0.14.1', 'telemetryChannel': 'release'}
        def require(ok, msg):
            if not ok:
                raise ValueError(msg)
        producer = SimpleNamespace(VERSION='0.14.1', require=require, compile_inputs=lambda: new)
        old = relink.previous_compile_inputs(producer)
        self.assertEqual(old, {'version': '0.14.0', 'versionStamp': '0.14.0', 'telemetryChannel': 'release'})
        self.assertEqual(new['version'], '0.14.1')
        producer.VERSION = '0.14.0'
        with self.assertRaises(ValueError):
            relink.previous_compile_inputs(producer)

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
            log = root / 'version.log'; log.write_text('backplane 0.14.1\n')
            data = {'BUILD.json': {'source_commit': relink.HOTFIX_COMMIT, 'source_tree': relink.HOTFIX_TREE,
                    'compileInputs': {'version': '0.14.1'},
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
            producer = SimpleNamespace(VERSION='0.14.1', require=require,
                compile_inputs=lambda: {'version': '0.14.1'},
                read=lambda p: data[p.name],
                digest=lambda p: 'newhash' if p.name == 'backplane-serve' else 'oldqual',
                inventory=lambda p: data['RUNTIME-QUALIFICATION.json']['members'],
                relative=lambda p: p,
                reference=lambda p: {'path': str(p)},
                write=lambda p, v: p.write_text(json.dumps(v)),
                runtime_closure=closure, component=combine)
            args = SimpleNamespace(target='linux-x64', build=build, previous=root / 'previous',
                source=relink.HOTFIX_COMMIT, launch_log=log, out=root / 'assembled/component')
            with patch.object(platform, 'system', return_value='Linux'), \
                    patch.object(platform, 'machine', return_value='x86_64'):
                relink.component(args, producer, None)
            self.assertEqual(calls, [args.out / 'payload'])


if __name__ == '__main__':
    unittest.main()
