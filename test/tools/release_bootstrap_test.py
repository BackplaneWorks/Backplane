#!/usr/bin/env python3
"""Offline transfer guards; no network, emit, build, secret or platform job."""
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import zipfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('bootstrap', ROOT / 'scripts/release-bootstrap.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
SOURCE, TREE = 'a' * 40, 'b' * 40


class BootstrapTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.inputs = self.root / 'inputs'
        self.inputs.mkdir()
        (self.inputs / 'js').mkdir()
        self.js = {'schema': 1, 'kind': 'backplane-service-js-bootstrap-inputs', 'sourceCommit': SOURCE,
                   'sourceTree': TREE, 'version': '0.12.1', 'sourceQualification': 'js/SOURCE-QUALIFICATION.json',
                   'webNotices': 'js/WEB-NOTICES.json', 'noticeInputs': {'build/release-notice-inputs/Bend.txt': 'js/notices/Bend.txt'}}
        (self.inputs / 'js/JS-INPUTS.json').write_text(json.dumps(self.js))
        (self.inputs / 'js/SOURCE-QUALIFICATION.json').write_text('{"public": "qualification"}')
        (self.inputs / 'js/WEB-NOTICES.json').write_text('{"public": "notices"}')
        (self.inputs / 'js/notices').mkdir()
        (self.inputs / 'js/notices/Bend.txt').write_text('public external license')

    def packet(self, name='packet.tar.gz'):
        archive = self.root / name
        result = b.pack(self.inputs, archive, SOURCE, TREE, 'js')
        self.assertEqual(result['archiveSha256'], b.sha(archive))
        return archive

    def unpack(self):
        dest = self.root / 'unpacked'
        b.extract(self.packet(), dest)
        return dest

    def test_round_trip_and_identity(self):
        dest = self.unpack()
        self.assertEqual(b.verify(dest, SOURCE, TREE, 'js')['version'], '0.12.1')
        for args in [('c'*40, TREE, 'js'), (SOURCE, 'c'*40, 'js'), (SOURCE, TREE, 'android')]:
            with self.subTest(args=args), self.assertRaises(ValueError): b.verify(dest, *args)

    def test_round_trip_preserves_receipt_bound_regular_file_modes(self):
        modes = (0o600, 0o640, 0o644, 0o664, 0o700, 0o755, 0o775)
        for mode in modes:
            member = self.inputs / ('mode-' + oct(mode))
            member.write_bytes(b'public mode-bound artifact')
            member.chmod(mode)
        dest = self.unpack()
        for mode in modes:
            member = dest / ('mode-' + oct(mode))
            self.assertEqual(member.stat().st_mode & 0o777, mode)

    def test_privileged_or_world_writable_tar_mode_refused_before_extraction(self):
        for mode in (0o4755, 0o2644, 0o1644, 0o666):
            archive = self.root / (oct(mode) + '.tar.gz')
            with tarfile.open(archive, 'w:gz') as tar:
                member = tarfile.TarInfo('public'); member.mode = mode; member.size = 1
                tar.addfile(member, io.BytesIO(b'x'))
            dest = self.root / oct(mode)
            with self.subTest(mode=mode), self.assertRaises(ValueError): b.extract(archive, dest)
            self.assertFalse(dest.exists())

    def test_changed_missing_extra_bytes_refused(self):
        dest = self.unpack()
        c = dest / 'js/notices/Bend.txt'
        c.write_text('changed')
        with self.assertRaises(ValueError): b.verify(dest, SOURCE, TREE, 'js')
        c.unlink()
        with self.assertRaises(ValueError): b.verify(dest, SOURCE, TREE, 'js')
        c.write_text('public external license')
        (dest / 'extra').write_text('extra')
        with self.assertRaises(ValueError): b.verify(dest, SOURCE, TREE, 'js')

    def test_noncanonical_paths(self):
        for path in ('../x', '/x', 'x/../y', 'x\\y', 'C:/x', 'x//y', './x', ''):
            with self.subTest(path=path), self.assertRaises(ValueError): b.relative(path)

    def test_symlink_and_private_input_refused(self):
        p = self.inputs / 'linked'
        p.symlink_to('js/notices/Bend.txt')
        with self.assertRaises(ValueError): self.packet()
        p.unlink()
        (self.inputs / 'signer.p12').write_text('not a real key')
        with self.assertRaises(ValueError): self.packet()

    def test_tar_traversal_links_duplicate_and_special_refused(self):
        for label, names, type_ in [('up', ['../out'], tarfile.REGTYPE), ('link', ['link'], tarfile.SYMTYPE),
                                     ('duplicate', ['a','A'], tarfile.REGTYPE), ('fifo', ['pipe'], tarfile.FIFOTYPE)]:
            archive = self.root / (label + '.tar.gz')
            with tarfile.open(archive, 'w:gz') as tar:
                for name in names:
                    m = tarfile.TarInfo(name); m.type = type_
                    if type_ == tarfile.REGTYPE: m.size = 1
                    tar.addfile(m, io.BytesIO(b'x') if m.isfile() else None)
            with self.subTest(label=label), self.assertRaises(ValueError): b.extract(archive, self.root / label)
            self.assertFalse((self.root / label).exists())

    def test_existing_destination_not_reused(self):
        archive = self.packet()
        with self.assertRaises(ValueError): b.extract(archive, self.inputs)

    def rewrite(self, dest, value):
        r = json.loads((dest / 'BOOTSTRAP.json').read_text())
        e = dest / 'js/JS-INPUTS.json'; e.write_text(json.dumps(value))
        r['files']['js/JS-INPUTS.json'] = {'sha256': b.sha(e), 'bytes': e.stat().st_size}
        (dest / 'BOOTSTRAP.json').write_text(json.dumps(r))

    def test_wrong_or_escaping_js_inputs_not_promoted(self):
        for change in [{'version': '0.12.1-dev'}, {'sourceTree': 'c'*40}, {'kind': 'backplane-service-c-emission'},
                       {'webNotices': 'js/missing.json'}, {'sourceQualification': '../x'},
                       {'noticeInputs': {'src/server/main.bend': 'js/notices/Bend.txt'}},
                       {'noticeInputs': {'build/release-notice-inputs/x': 'js/notices/missing'}}]:
            n = str(len(list(self.root.iterdir())))
            dest = self.root / ('case-' + n)
            b.extract(self.packet('packet-' + n + '.tar.gz'), dest)
            self.rewrite(dest, {**self.js, **change})
            with self.subTest(change=change), self.assertRaises(ValueError): b.verify(dest, SOURCE, TREE, 'js')

    def test_every_js_target_has_a_pinned_runtime(self):
        self.assertEqual(set(b.BUN_RUNTIMES), {'linux-x64', 'linux-arm64', 'darwin-x64', 'darwin-arm64', 'windows-x64'})
        for stem, digest in b.BUN_RUNTIMES.values():
            self.assertRegex(digest, '^[0-9a-f]{64}$')
            self.assertTrue(stem.startswith('bun-'))

    def runtime_zip(self, target, body=b'runtime'):
        stem, _ = b.BUN_RUNTIMES[target]
        archive = self.root / (target + '.zip')
        with zipfile.ZipFile(archive, 'w') as z:
            z.writestr(stem + '/' + ('bun.exe' if target.startswith('windows') else 'bun'), body)
            z.writestr(stem + '/other', b'not taken')
        return archive

    def test_runtime_takes_only_the_pinned_zip_executable(self):
        for target in ('windows-x64', 'linux-arm64'):
            archive = self.runtime_zip(target)
            def fake(url, expected, out):
                self.assertEqual(url, 'https://github.com/oven-sh/bun/releases/download/bun-v1.4.3/' + b.BUN_RUNTIMES[target][0] + '.zip')
                self.assertEqual(expected, b.BUN_RUNTIMES[target][1])
                out.write_bytes(archive.read_bytes())
            with patch.object(b, 'download', side_effect=fake):
                got = b.bun_runtime(target, self.root / ('rt-' + target))
            exe = self.root / ('rt-' + target) / ('bun.exe' if target == 'windows-x64' else 'bun')
            self.assertEqual(got['path'], str(exe))
            self.assertEqual((got['target'], got['version'], got['sha256']), (target, '1.4.3', b.sha(exe)))
            self.assertEqual(sorted(p.name for p in exe.parent.iterdir()), [exe.name])
        with self.assertRaises(ValueError): b.bun_runtime('windows-arm64', self.root / 'rt-bad')

    def test_run_calls_only_checked_out_js_producer(self):
        dest = self.unpack()
        product = self.root / 'product'
        product.mkdir()
        def tool(name, flag):
            return {'path': '/' + name, 'sha256': 'f'*64, 'bytes': 1, 'version': '1.4.3' if name == 'bun' else '2.0.36'}
        runtime = {'path': '/rt/bun', 'sha256': 'e'*64, 'bytes': 1, 'version': '1.4.3', 'target': 'linux-arm64'}
        with patch.object(b,'source_identity',return_value=TREE), patch.object(b,'tool',side_effect=tool), \
             patch.object(b,'bun_runtime',return_value=runtime), patch.object(b.subprocess,'run') as run:
            b.run(dest,'js','linux-arm64',SOURCE,product)
            args=run.call_args.args[0]
            self.assertEqual(args[1:3],[str(ROOT/'scripts/service-release.py'),'js'])
            self.assertEqual(args[3:5],['--source-root',str(product)])
            self.assertEqual(args[-1],str(product/'build/bootstrap-js'))
            self.assertEqual(run.call_args.kwargs['cwd'],product)
            request=json.loads((dest.parent/'js-linux-arm64.json').read_text())
            self.assertEqual((request['kind'], request['target']), ('backplane-service-js-inputs', 'linux-arm64'))
            self.assertEqual(request['toolchain']['bunRuntime'], runtime)
            self.assertEqual(request['webNotices']['sha256'], b.sha(dest/'js/WEB-NOTICES.json'))
            self.assertEqual((product/'build/release-notice-inputs/Bend.txt').read_text(), 'public external license')
            self.assertNotIn('clang', json.dumps(request))
        with patch.object(b,'source_identity',return_value=TREE), self.assertRaises(ValueError):
            b.run(dest,'js','windows-arm64',SOURCE,product)
        (product/'build/release-notice-inputs/Bend.txt').write_text('changed')
        (dest.parent/'js-linux-arm64.json').unlink()
        with patch.object(b,'source_identity',return_value=TREE), patch.object(b,'tool',side_effect=tool), \
             patch.object(b,'bun_runtime',return_value=runtime), patch.object(b.subprocess,'run'), self.assertRaises(ValueError):
            b.run(dest,'js','linux-arm64',SOURCE,product)

    def test_wrong_bun_refused(self):
        dest = self.unpack()
        with patch.object(b,'source_identity',return_value=TREE), \
             patch.object(b,'tool',return_value={'path':'/bun','sha256':'f'*64,'bytes':1,'version':'1.3.10'}), \
             patch.object(b.subprocess,'run') as run, self.assertRaisesRegex(ValueError, 'Bun 1.4.3'):
            b.run(dest,'js','linux-x64',SOURCE,self.root / 'product')
        run.assert_not_called()

    def test_no_c_operation_remains(self):
        self.assertEqual(b.OPERATIONS, ('js', 'android'))
        text = (ROOT/'.github/workflows/release-bootstrap.yml').read_text()
        for word in ('clang', 'unix-native', 'windows-native', 'release-llvm', 'cygwin'):
            self.assertFalse(word in text.lower(), word)

    def test_workflow_pins_this_component_producer(self):
        text = (ROOT/'.github/workflows/release-bootstrap.yml').read_text()
        self.assertTrue('--producer-sha256 ' + b.sha(ROOT/'scripts/service-release.py') in text, 'stale producer pin')

    def test_android_executes_separately_pinned_metadata_without_mutating_packet(self):
        request=self.inputs/'android-release-inputs.json'
        request.write_text(json.dumps({'sourceCommit':SOURCE,'sourceTree':TREE}))
        original=request.read_bytes()
        archive=self.root/'android.tar.gz'
        b.pack(self.inputs,archive,SOURCE,TREE,'android')
        dest=self.root/'android-inputs'; b.extract(archive,dest)
        product=self.root/'separate frozen product'
        with patch.object(b,'source_identity',return_value=TREE), patch.object(b.subprocess,'run') as run:
            b.run(dest,'android',None,SOURCE,product)
            args=run.call_args.args[0]
            helper=ROOT/'scripts/build-android-artifacts.py'
            self.assertEqual(args[1:4],[str(helper),'--execution-helper-sha256',b.sha(helper)])
            self.assertEqual(args[args.index('--source')+1],str(product))
            self.assertEqual((dest/'android-release-inputs.json').read_bytes(),original)
            self.assertEqual(run.call_args.kwargs['cwd'],product)

    def test_existing_and_bootstrap_android_use_real_producer_secret_names(self):
        for name in ('release.yml', 'release-bootstrap.yml'):
            text=(ROOT/'.github/workflows'/name).read_text()
            for key,secret in [('KEYSTORE_PASSWORD','BACKPLANE_ANDROID_KEYSTORE_PASSWORD'),
                               ('KEY_ALIAS','BACKPLANE_ANDROID_KEY_ALIAS'),
                               ('KEY_PASSWORD','BACKPLANE_ANDROID_KEY_PASSWORD')]:
                self.assertIn(key+': ${{ secrets.'+secret+' }}',text)
                self.assertNotIn('          '+secret+':',text)

    def test_execution_records_distinct_metadata_and_product_identity(self):
        with patch.object(b.subprocess,'check_output',return_value='d'*40+'\n'), patch.dict(b.os.environ, {'GITHUB_SHA':'d'*40}):
            r=b.record_execution(self.root,SOURCE,TREE)
            self.assertEqual(r['workflowSourceCommit'],'d'*40)
            self.assertEqual(r['productSourceCommit'],SOURCE)
            self.assertFalse(r['configuredBuildAccepted'])
            self.assertEqual(b.record_execution(self.root,SOURCE,TREE),r)
        with patch.object(b.subprocess,'check_output',return_value='e'*40+'\n'), patch.dict(b.os.environ, {'GITHUB_SHA':'d'*40}), self.assertRaises(ValueError):
            b.record_execution(self.root,SOURCE,TREE)

    def test_https_without_credentials_only(self):
        for url in ('http://example.com/x','https://user:pass@example.com/x','https://example.com:8443/x','https://example.com/x#fragment'):
            with self.subTest(url=url), self.assertRaises(ValueError): b.fetch(url,'c'*64,self.root/'download',SOURCE,TREE,'js')

    def test_wrong_download_digest_never_extracts(self):
        archive=self.packet()
        class Response(io.BytesIO):
            url='https://example.com/packet'
        with patch.object(b.urllib.request,'urlopen',return_value=Response(archive.read_bytes())), self.assertRaises(ValueError):
            b.fetch('https://example.com/packet','c'*64,self.root/'download',SOURCE,TREE,'js')
        self.assertFalse((self.root/'download').exists())


if __name__=='__main__': unittest.main()
