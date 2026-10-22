#!/usr/bin/env python3
"""Actual release package/installer paths in disposable prefixes, no network."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

SOURCE = Path(__file__).parents[2]
spec = importlib.util.spec_from_file_location('cef_runtime', SOURCE / 'scripts/cef-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class PublicRelease(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='cef-public-release-')
        self.root = Path(self.temp.name)
        self.repo = self.root / 'repo'
        for directory in ('scripts', 'deploy', 'assets/icon', 'dist/web', 'bin', 'home', 'cache'):
            (self.repo / directory).mkdir(parents=True)
        for file in ('package.sh', 'package-linux.sh', 'install-release.sh', 'cef-runtime.py'):
            shutil.copy2(SOURCE / 'scripts' / file, self.repo / 'scripts' / file)
        shutil.copy2(SOURCE / 'deploy/backplane.desktop', self.repo / 'deploy/backplane.desktop')
        shutil.copytree(SOURCE / 'assets/icon', self.repo / 'assets/icon', dirs_exist_ok=True)
        (self.repo / 'LICENSE').write_text('App license fixture')
        (self.repo / 'dist/web/index.html').write_text('web-first')
        for file in ('backplane', 'backplane-serve'):
            shutil.copy2('/bin/true', self.repo / 'dist' / file)
        self.prefix = self.root / 'prefix'
        self.home = self.repo / 'home'
        private = self.home / '.backplane-bend'
        private.mkdir()
        (private / 'events.jsonl').write_text('explicit browser.engine=chrome; session sentinel\n')
        self.env = dict(os.environ, HOME=str(self.home), XDG_CACHE_HOME=str(self.repo / 'cache'),
                        BACKPLANE_PREFIX=str(self.prefix), BACKPLANE_BIN=str(self.root / 'links'),
                        PATH=str(self.repo / 'bin') + ':' + os.environ['PATH'])
        self.build_payload('first')

    def tearDown(self):
        self.temp.cleanup()

    def build_payload(self, label):
        dist = self.repo / 'dist'
        (dist / 'web').mkdir(exist_ok=True)
        (dist / 'web/index.html').write_text('web-' + label)
        (dist / 'kistack/skills').mkdir(parents=True, exist_ok=True)
        (dist / 'kistack/skills/README.md').write_text('skills-' + label)
        helper = dist / 'backplane-browser'
        helper.write_text('#!/bin/sh\necho helper-' + label + '\n')
        helper.chmod(0o755)
        stamp = dist / 'backplane-browser.stamp'
        stamp.write_text(label)
        native = self.root / ('native-' + label)
        native.mkdir()
        host = native / 'backplane-cef'
        host.write_text('#!/bin/sh\necho host-' + label + '\n')
        host.chmod(0o755)
        # ELF library fixture lets the real package-linux objdump inspect it.
        shutil.copy2('/bin/true', native / 'libcef.so')
        (native / 'icudtl.dat').write_text('icu fixture')
        (native / 'LICENSE.txt').write_text('CEF license fixture')
        (native / 'CEF-README.txt').write_text('CEF redistribution fixture')
        shutil.rmtree(dist / 'browser-runtime', ignore_errors=True)
        runtime.assemble(dist / 'browser-runtime', native, host, helper, stamp, label)

    def package(self):
        subprocess.run(['sh', 'scripts/package.sh', 'v9.0.0-test', 'linux-x64'], cwd=self.repo,
                       env=self.env, check=True, capture_output=True)
        return self.repo / 'dist/pkg/backplane-linux-x64'

    def install(self):
        self.package()
        archive = self.repo / 'dist/backplane-v9.0.0-test-linux-x64-cef.tar.gz'
        # the public installer takes it under its classic name, hub only (no desktop app here)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        script = (SOURCE / 'deploy/install.sh.in').read_text().replace('@VERSION@', 'v9.0.0-test').replace('@SHA_LINUX_X64@', digest)
        installer = self.root / 'install.sh'
        installer.write_text(script)
        # The real installer still verifies SHA256 and extracts the real archive.
        curl = self.repo / 'bin/curl'
        curl.write_text('#!/bin/sh\nwhile [ "$#" -gt 0 ]; do case "$1" in -o) out=$2; shift 2;; *) url=$1; shift;; esac; done\n'
                        'case "$url" in */backplane-v9.0.0-test-classic-linux-x64.tar.gz) ;; *) exit 2;; esac\n'
                        'cp "$TEST_RELEASE_ARCHIVE" "$out"\n')
        curl.chmod(0o755)
        env = dict(self.env, TEST_RELEASE_ARCHIVE=str(archive), BACKPLANE_NO_DESKTOP_APP='1')
        subprocess.run(['sh', str(installer)], env=env, check=True, capture_output=True)

    def update(self, package):
        binary = os.environ.get('CEF_UPDATE_NATIVE')
        cmd = [binary, str(package), str(self.prefix)] if binary else ['sh', str(SOURCE / 'scripts/install-release.sh'), str(package), str(self.prefix)]
        result = subprocess.run(cmd, env=self.env, text=True, capture_output=True, check=False)
        return result.returncode == 0 and not result.stdout.strip(), result.stdout + result.stderr

    def pair(self):
        return ((self.prefix / 'backplane-browser').read_text(), (self.prefix / 'cef/backplane-cef').read_text())

    def test_rendered_clean_installer_and_native_update_preserve_session_and_rollback(self):
        self.install()
        self.assertIn('helper-first', self.pair()[0])
        self.assertIn('host-first', self.pair()[1])
        self.assertEqual((self.prefix / 'kistack/skills/README.md').read_text(), 'skills-first')
        first = (self.prefix / 'browser-current').resolve()
        self.build_payload('second')
        ok, output = self.update(self.package())
        self.assertTrue(ok, output)
        self.assertIn('helper-second', self.pair()[0])
        self.assertIn('host-second', self.pair()[1])
        self.assertEqual((self.prefix / 'kistack/skills/README.md').read_text(), 'skills-second')
        self.assertEqual((self.prefix / 'kistack.old/skills/README.md').read_text(), 'skills-first')
        self.assertEqual((self.prefix / 'browser-previous').resolve(), first)
        runtime.rollback(self.prefix)
        self.assertIn('helper-first', self.pair()[0])
        self.assertEqual((self.home / '.backplane-bend/events.jsonl').read_text(), 'explicit browser.engine=chrome; session sentinel\n')

    def test_corrupt_update_refused_before_app_or_pair_change(self):
        self.install()
        before = self.pair()
        app = hashlib.sha256((self.prefix / 'backplane').read_bytes()).hexdigest()
        self.build_payload('damaged')
        package = self.package()
        (package / 'browser-runtime/cef/libcef.so').write_text('corrupted')
        ok, output = self.update(package)
        self.assertFalse(ok, output)
        self.assertIn('hash mismatch', output)
        self.assertEqual(before, self.pair())
        self.assertEqual(app, hashlib.sha256((self.prefix / 'backplane').read_bytes()).hexdigest())
        self.assertEqual((self.prefix / 'kistack/skills/README.md').read_text(), 'skills-first')
        self.assertFalse((self.prefix / 'kistack.old').exists())

    def test_package_refuses_unmatched_helper_and_missing_runtime(self):
        (self.repo / 'dist/backplane-browser').write_text('unmatched')
        result = subprocess.run(['sh', 'scripts/package.sh', 'v9', 'linux-x64'], cwd=self.repo, env=self.env, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        shutil.rmtree(self.repo / 'dist/browser-runtime')
        result = subprocess.run(['sh', 'scripts/package.sh', 'v9', 'linux-x64'], cwd=self.repo, env=self.env, capture_output=True)
        self.assertNotEqual(result.returncode, 0)

    def test_non_native_archive_stays_plain_and_leaves_existing_cef_pair(self):
        self.install()
        before = self.pair()
        shutil.rmtree(self.repo / 'dist/browser-runtime')
        subprocess.run(['sh', 'scripts/package.sh', 'v9', 'linux-arm64'], cwd=self.repo,
                       env=self.env, check=True, capture_output=True)
        self.assertTrue((self.repo / 'dist/backplane-v9-linux-arm64.tar.gz').is_file())
        # 0.12.1 on: a release without CEF updates and leaves the old pair alone
        ok, output = self.update(self.repo / 'dist/pkg/backplane-linux-arm64')
        self.assertTrue(ok, output)
        self.assertEqual(self.pair(), before)

    def test_managed_packages_carry_pair_and_runtime_dependencies(self):
        package = self.package()
        tool = self.repo / 'bin/appimagetool'
        tool.write_text('#!/bin/sh\nfor out do :; done\n: > "$out"\n')
        tool.chmod(0o755)
        env = dict(self.env, APPIMAGETOOL=str(tool))
        subprocess.run(['sh', 'scripts/package-linux.sh', 'v9.0.0-test', 'linux-x64'], cwd=self.repo, env=env, check=True, capture_output=True)
        appdir = self.repo / 'build/pkg-linux/AppDir/usr/lib/backplane'
        runtime.check_release(appdir)
        self.assertTrue((appdir / 'cef/backplane-cef').is_file())
        deb = next((self.repo / 'dist').glob('*.deb'))
        control = subprocess.check_output(['ar', 'p', str(deb), 'control.tar.gz'])
        with tarfile.open(fileobj=io.BytesIO(control)) as tar:
            text = tar.extractfile('./control').read().decode()
        self.assertIn('libnss3', text)
        self.assertIn('libasound2', text)
        self.assertTrue((package / 'browser-runtime/cef/LICENSE.txt').is_file())


if __name__ == '__main__':
    unittest.main(verbosity=2)
