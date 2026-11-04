#!/usr/bin/env python3
"""Negative and delegation controls for exact configured-service reuse."""
import importlib.util
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('relink', ROOT / 'scripts/service-relink.py')
relink = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relink)


class RelinkTests(unittest.TestCase):
    def require(self, ok, message):
        if not ok:
            raise ValueError(message)

    def test_actual_qualified_old_source_and_actual_root_new_source(self):
        directory = os.environ.get('BACKPLANE_RELINK_AUDIT')
        self.assertTrue(directory, 'Actual source receipts required for release controls')
        root = Path(directory)
        old = json.loads((root / 'OLD-SOURCE.json').read_text())
        prior = json.loads((root / 'OLD-BUILD.json').read_text())
        new = json.loads((root / 'NEW-SOURCE.json').read_text())
        emitted = json.loads((root / 'NEW-EMISSION.json').read_text())
        self.assertEqual(hashlib.sha256((root / 'OLD-SOURCE.json').read_bytes()).hexdigest(),
                         prior['sourceManifestSha256'])
        self.assertEqual(prior['source_commit'], relink.BASE_COMMIT)
        self.assertEqual(prior['source_tree'], relink.BASE_TREE)
        self.assertEqual(new['commit'], relink.HOTFIX_COMMIT)
        self.assertEqual(new['tree'], relink.HOTFIX_TREE)
        self.assertEqual(hashlib.sha256((root / 'NEW-SOURCE.json').read_bytes()).hexdigest(),
                         emitted['files']['SOURCE.json']['sha256'])
        self.assertEqual(relink.source_equivalence(old['members'], new['members']),
                         sorted(relink.REVIEWED_DELTA))
        prior_emission = json.loads((root / 'OLD-EMISSION.json').read_text())
        relink.validate_raw_change(SimpleNamespace(require=self.require), emitted, prior_emission)

    def test_same_old_raw_server_and_changed_web_rejected(self):
        producer = SimpleNamespace(require=self.require)
        old = {'files': {'server.js': {'sha256': 'old'}, 'web/index.html': {'sha256': 'same'}}}
        new = {'files': {'server.js': {'sha256': 'new'}, 'web/index.html': {'sha256': 'same'}}}
        relink.validate_raw_change(producer, new, old)
        with self.assertRaises(ValueError):
            relink.validate_raw_change(producer, old, old)
        new['files']['web/index.html'] = {'sha256': 'unreviewed'}
        with self.assertRaises(ValueError):
            relink.validate_raw_change(producer, new, old)

    def test_prior_compile_input_version_and_channel_mismatches_rejected(self):
        producer = SimpleNamespace(VERSION='0.14.1', require=self.require,
            compile_inputs=lambda: {'version': '0.14.1', 'versionStamp': '0.14.1',
                                    'telemetryChannel': 'release'})
        expected = relink.previous_compile_inputs(producer)
        relink.validate_previous_compile_inputs(producer, {'compileInputs': expected})
        for name, value in [('version', '0.14.1'), ('versionStamp', '0.14.1'),
                            ('telemetryChannel', 'dev')]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                relink.validate_previous_compile_inputs(producer,
                    {'compileInputs': {**expected, name: value}})

    def test_actual_component_runtime_layout_is_recovered_exactly(self):
        root = Path(os.environ['BACKPLANE_RELINK_AUDIT'])
        producer = relink.load('runtime_producer', ROOT / 'scripts/service-release.py')
        qualification = producer.read(root / 'OLD-RUNTIME-QUALIFICATION.json')
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'runtime'
            relink.copy_retained_runtime(producer, root / 'runtime-payload', qualification, output)
            self.assertEqual(producer.inventory(output), qualification['members'])
            with self.assertRaises(ValueError):
                relink.copy_retained_runtime(producer, root / 'runtime-payload', qualification, output)

    def test_runtime_member_hash_mismatch_is_rejected(self):
        producer = relink.load('runtime_negative_producer', ROOT / 'scripts/service-release.py')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            payload = root / 'payload'
            payload.mkdir()
            (payload / 'lib').write_bytes(b'tampered')
            qualification = {'members': {'lib': {'type': 'file', 'sha256': '0' * 64,
                                                'bytes': 8, 'mode': 0o644}}}
            with self.assertRaises(ValueError):
                relink.copy_retained_runtime(producer, payload, qualification, root / 'runtime')

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
                    patch.object(platform, 'machine', return_value='x86_64'), \
                    patch.object(relink, 'copy_retained_runtime', return_value=runtime):
                relink.component(args, producer, None)
            self.assertEqual(calls, [args.out / 'payload'])


if __name__ == '__main__':
    unittest.main()
