#!/usr/bin/env python3
"""Exercise deployment filesystem effects on isolated prefixes, no live data."""
import importlib.util
import os
import json
import subprocess
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('cef_runtime', Path(__file__).parents[2] / 'scripts/cef-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class Lifecycle(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='cef-runtime-test-')
        self.root = Path(self.temp.name)
        self.prefix = self.root / 'install'
        self.prefix.mkdir()
        self.cache = self.root / 'cache'
        self.env = patch.dict(os.environ, {'XDG_CACHE_HOME': str(self.cache)})
        self.env.start()
        self.old = self.cache / 'backplane/cef-runtime' / runtime.CEF_VERSION
        self.old.mkdir(parents=True)
        for name, text in [('backplane-cef', '#!/bin/sh\necho host-old\n'), ('libcef.so', 'pinned-library'), ('icudtl.dat', 'icu')]:
            (self.old / name).write_text(text)
        (self.old / runtime.HOST).chmod(0o755)
        self.helper(self.prefix / runtime.HELPER, 'helper-old')
        (self.prefix / runtime.STAMP).write_text('old\n')
        (self.root / 'LICENSE.txt').write_text('pinned CEF license fixture')
        (self.root / 'CEF-README.txt').write_text('upstream redistribution instructions fixture')

    def tearDown(self):
        self.env.stop()
        self.temp.cleanup()

    def helper(self, file, label):
        file.write_text('#!/bin/sh\necho ' + label + '\n')
        file.chmod(0o755)

    def package(self, label):
        package = self.root / label
        package.mkdir()
        self.helper(package / runtime.HELPER, 'helper-' + label)
        (package / runtime.STAMP).write_text(label + '\n')
        host = self.root / ('host-' + label)
        self.helper(host, 'host-' + label)
        runtime.assemble(package / 'browser-runtime', self.old, host, package / runtime.HELPER, package / runtime.STAMP, label)
        return package

    def pair(self):
        return ((self.prefix / runtime.HELPER).read_text(), (self.prefix / 'cef' / runtime.HOST).read_text())

    def test_install_update_rollback_retains_legacy_and_old_bundle(self):
        first = self.package('first')
        runtime.install(first, self.prefix)
        self.assertEqual(self.pair(), ('#!/bin/sh\necho helper-first\n', '#!/bin/sh\necho host-first\n'))
        legacy = (self.prefix / 'browser-previous').resolve()
        runtime.verify(legacy)
        self.assertIn('helper-old', (legacy / runtime.HELPER).read_text())
        self.assertIn('host-old', (legacy / 'cef' / runtime.HOST).read_text())
        first_dir = (self.prefix / 'browser-current').resolve()
        second = self.package('second')
        runtime.install(second, self.prefix)
        self.assertIn('helper-second', self.pair()[0])
        self.assertIn('host-second', self.pair()[1])
        self.assertTrue(first_dir.is_dir())
        self.assertTrue(legacy.is_dir())
        self.assertTrue((self.old / runtime.HOST).is_file())
        runtime.rollback(self.prefix)
        self.assertEqual((self.prefix / 'browser-current').resolve(), first_dir)
        self.assertIn('host-first', self.pair()[1])
        self.assertFalse((second / runtime.HELPER).exists())

    def test_fresh_bundle_keeps_notices_when_existing_runtime_lacks_them(self):
        package = self.package('notices')
        payload = package / 'browser-runtime'
        runtime.verify(payload)
        self.assertEqual((payload / 'cef/LICENSE.txt').read_text(), 'pinned CEF license fixture')
        self.assertTrue((payload / 'cef/CEF-README.txt').is_file())
        manifest = json.loads((payload / 'manifest.json').read_text())
        self.assertIn('cef/LICENSE.txt', manifest['files'])
        self.assertIn('cef/CEF-README.txt', manifest['files'])

    def test_missing_notice_refuses_fresh_stage_without_partial_payload(self):
        (self.root / 'LICENSE.txt').unlink()
        with self.assertRaisesRegex(ValueError, 'missing notice'):
            self.package('missing-notice')
        self.assertFalse((self.root / 'missing-notice/browser-runtime').exists())
        self.assertIn('helper-old', (self.prefix / runtime.HELPER).read_text())

    def test_no_runtime_does_not_opt_in(self):
        package = self.package('first')
        shutil.rmtree(self.old)
        runtime.install(package, self.prefix)
        self.assertIsNone(runtime.discover(self.prefix))
        self.assertFalse((self.prefix / 'browser-current').exists())
        self.assertFalse((self.prefix / 'browser-runtimes').exists())
        self.assertIn('helper-old', (self.prefix / runtime.HELPER).read_text())
        self.assertTrue((package / runtime.HELPER).exists())

    def test_missing_optional_payload_is_a_no_op(self):
        runtime.install(self.root / 'absent-package', self.prefix)
        self.assertIn('helper-old', (self.prefix / runtime.HELPER).read_text())
        self.assertFalse((self.prefix / 'browser-current').exists())

    def test_public_install_opts_in_clean_prefix_and_keeps_previous_pair(self):
        shutil.rmtree(self.old)
        (self.prefix / runtime.HELPER).unlink()
        # The staged runtime still comes from a separately prepared build.
        self.old.mkdir(parents=True)
        for name in ('backplane-cef', 'libcef.so', 'icudtl.dat'):
            (self.old / name).write_text('runtime fixture')
        first = self.package('public-first')
        shutil.rmtree(self.old)
        runtime.install(first, self.prefix, enable=True)
        self.assertIn('helper-public-first', self.pair()[0])
        self.assertIn('host-public-first', self.pair()[1])
        self.assertFalse((self.prefix / 'browser-previous').exists())
        # A second matched public payload retains a rollback pair.
        first_payload = (self.prefix / 'browser-current').resolve()
        second = self.root / 'public-second'
        second.mkdir()
        shutil.copytree(first_payload, second / 'browser-runtime')
        (second / 'browser-runtime' / runtime.HELPER).write_text('#!/bin/sh\necho second\n')
        (second / 'browser-runtime' / 'manifest.json').unlink()
        runtime.seal(second / 'browser-runtime', 'public-second')
        runtime.install(second, self.prefix, enable=True)
        self.assertEqual((self.prefix / 'browser-previous').resolve(), first_payload)
        runtime.rollback(self.prefix)
        self.assertIn('helper-public-first', self.pair()[0])

    def test_corruption_and_mismatched_helper_leave_current_pair(self):
        runtime.install(self.package('first'), self.prefix)
        before = self.pair()
        target = (self.prefix / 'browser-current').readlink()
        damaged = self.package('damaged')
        (damaged / 'browser-runtime/cef/libcef.so').write_text('corrupt')
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            runtime.install(damaged, self.prefix)
        mismatch = self.package('mismatch')
        (mismatch / runtime.HELPER).write_text('unmatched helper')
        with self.assertRaisesRegex(ValueError, 'differs'):
            runtime.install(mismatch, self.prefix)
        self.assertEqual(before, self.pair())
        self.assertEqual(target, (self.prefix / 'browser-current').readlink())

    def test_copy_failure_leaves_current_pair(self):
        runtime.install(self.package('first'), self.prefix)
        before = self.pair()
        package = self.package('next')
        with patch.object(runtime.shutil, 'copytree', side_effect=OSError('disk full')):
            with self.assertRaisesRegex(OSError, 'disk full'):
                runtime.install(package, self.prefix)
        self.assertEqual(before, self.pair())
        self.assertFalse(list((self.prefix / 'browser-runtimes').glob('.stage-*')))

    def test_actual_hub_and_dev_swap_scripts_activate_matching_pair(self):
        source = Path(__file__).parents[2]
        hub = (source / 'src/server/server.bend').read_text().split('def Deploy.swap.sh() -> String:\n', 1)[1].splitlines()[0].strip()
        hub = json.loads(hub)
        dev = (source / 'scripts/dev.sh').read_text().split("install_into() {\n  cat <<'EOF'\n", 1)[1].split('\nEOF', 1)[0]
        for name, script in [('hub', hub), ('dev', dev)]:
            with self.subTest(path=name):
                package = self.package(name)
                shutil.copy2(runtime.__file__, package / 'cef-runtime.py')
                (package / 'web').mkdir()
                (package / 'web/index.html').write_text(name)
                (package / 'backplane').write_text('app-' + name)
                home = self.root / 'home'
                home.mkdir(exist_ok=True)
                env = dict(os.environ, HOME=str(home))
                subprocess.run(['sh', '-c', script, 'sh', str(package), str(self.prefix)], env=env, check=True, capture_output=True)
                self.assertIn('helper-' + name, self.pair()[0])
                self.assertIn('host-' + name, self.pair()[1])
                self.assertEqual((self.prefix / 'web/index.html').read_text(), name)
                self.assertEqual((self.prefix / 'backplane').read_text(), 'app-' + name)
                broken = self.package(name + '-broken')
                shutil.copy2(runtime.__file__, broken / 'cef-runtime.py')
                (broken / 'browser-runtime/cef/libcef.so').write_text('corrupt')
                (broken / 'backplane').write_text('bad-app')
                result = subprocess.run(['sh', '-c', script, 'sh', str(broken), str(self.prefix)], env=env, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual((self.prefix / 'backplane').read_text(), 'app-' + name)
                self.assertIn('host-' + name, self.pair()[1])

    def test_unlisted_files_or_symlink_payload_are_rejected(self):
        package = self.package('extra')
        (package / 'browser-runtime/extra').write_text('unexpected')
        with self.assertRaisesRegex(ValueError, 'file set'):
            runtime.install(package, self.prefix)
        package = self.package('linked')
        (package / 'browser-runtime/link').symlink_to('/etc/passwd')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            runtime.install(package, self.prefix)
        self.assertIn('helper-old', (self.prefix / runtime.HELPER).read_text())


if __name__ == '__main__':
    unittest.main(verbosity=2)
