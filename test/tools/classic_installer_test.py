#!/usr/bin/env python3
"""Exercise generated installer checksums and asset selection, without installing."""
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
REPO = 'BackplaneWorks/Backplane'
URL = f'https://github.com/{REPO}/releases/latest/download/install.sh'
TARGETS = ['linux-x64', 'linux-arm64', 'darwin-arm64']


class ClassicInstallerTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.out = self.root / 'out'
        self.inputs = []
        for target in TARGETS:
            service = self.root / target / 'service'
            (service / 'web').mkdir(parents=True)
            (service / 'web/index.html').write_text('fixture')
            (service / 'backplane-serve').write_text('#!/bin/sh\nexit 91\n')
            (service / 'backplane-serve').chmod(0o755)
            (service / 'LICENSE').write_text('fixture')
            app = self.root / target / ('backplane-' + target)
            app.mkdir()
            (app / 'desktop-fixture').write_text('fixture')
            archive = self.root / f'backplane-v9.0.0-{target}.tar.gz'
            with tarfile.open(archive, 'w:gz') as t:
                t.add(app, arcname=app.name)
            self.inputs.extend([target, str(service), str(archive)])

    def generate(self, inputs=None):
        return subprocess.run(['sh', str(ROOT / 'scripts/release-classic.sh'), 'v9.0.0', str(self.out),
                               *(self.inputs if inputs is None else inputs)], cwd=ROOT, text=True, capture_output=True)

    def test_public_entrypoints_use_the_current_repository(self):
        for name in ['README.md', 'docs/install.md', 'deploy/install.sh.in']:
            text = (ROOT / name).read_text()
            self.assertIn(URL, text, name)
            self.assertNotIn('backplane-bend/releases/', text, name)
        self.assertIn(f'REPO="{REPO}"', (ROOT / 'deploy/install.sh.in').read_text())

    def test_missing_apple_silicon_never_generates_a_public_installer(self):
        result = self.generate(self.inputs[:6])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('missing required installer target darwin-arm64', result.stderr)
        self.assertFalse((self.out / 'install.sh').exists())

    def test_every_platform_has_a_real_matching_classic_asset(self):
        result = self.generate()
        self.assertEqual(result.returncode, 0, result.stderr)
        script = (self.out / 'install.sh').read_text()
        self.assertNotIn('@SHA_', script)
        for target in TARGETS:
            archive = self.out / f'backplane-v9.0.0-classic-{target}.tar.gz'
            key = 'SHA_' + target.upper().replace('-', '_')
            self.assertIn(f'{key}="{hashlib.sha256(archive.read_bytes()).hexdigest()}"', script)
            with tarfile.open(archive) as t:
                self.assertIn(f'backplane-{target}/desktop/desktop-fixture', t.getnames())
        # Run the actual generated Darwin selector, stopping at the first network
        # call. No extraction, install, launch or remote mutation can occur.
        tools = self.root / 'tools'
        tools.mkdir()
        for name, body in {'uname': 'case "$1" in -s) echo Darwin;; -m) echo arm64;; esac',
                           'curl': 'printf "%s\\n" "$@" > "$TEST_FETCH_ARGS"; exit 37'}.items():
            p = tools / name
            p.write_text('#!/bin/sh\n' + body + '\n')
            p.chmod(0o755)
        record = self.root / 'fetch.txt'
        env = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'],
               'HOME': str(self.root / 'home'), 'BACKPLANE_PREFIX': str(self.root / 'prefix'),
               'BACKPLANE_BIN': str(self.root / 'bin'), 'TEST_FETCH_ARGS': str(record)}
        run = subprocess.run(['sh', str(self.out / 'install.sh')], env=env, text=True, capture_output=True)
        self.assertNotEqual(run.returncode, 0)
        self.assertIn(f'https://github.com/{REPO}/releases/download/v9.0.0/backplane-v9.0.0-classic-darwin-arm64.tar.gz', record.read_text())
        self.assertNotIn('no build', run.stderr)


if __name__ == '__main__':
    unittest.main()
