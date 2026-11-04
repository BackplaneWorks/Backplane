#!/usr/bin/env python3
"""Exercise production discovery with real filesystem entries and a Windows uname.

Runs on Boat/POSIX; PE execution and Windows filesystem semantics are not claimed.
"""
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Discovery(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.output = tempfile.TemporaryDirectory(prefix='bp-discovery-emit-')
        cls.addClassCleanup(cls.output.cleanup)
        cls.program = Path(cls.output.name) / 'discovery.js'
        cls.bun = shutil.which('bun')
        if not cls.bun:
            raise RuntimeError('Bun is required for production filesystem discovery')
        subprocess.run(['bend', str(ROOT / 'test/native/kicad_discovery.bend'), '-o', str(cls.program)],
                       cwd=ROOT, check=True, timeout=90)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='bp-discovery-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        fake = self.root / 'commands'; fake.mkdir()
        uname = fake / 'uname'
        uname.write_text('#!/bin/sh\nprintf "%s\\n" "CYGWIN_NT-10.0 x86_64"\n')
        uname.chmod(0o755)
        (fake / 'env').symlink_to(shutil.which('env'))
        self.env = dict(os.environ, PATH=str(fake), HOME=str(self.root / 'user'),
                        TEST_KICAD_ROOT=str(self.root), BACKPLANE_KICAD='', BACKPLANE_KICAD_CLI='',
                        ProgramFiles=str(self.root / 'Program Files'))
        self.runtime('old')
        self.runtime('posix', exe=False)
        self.runtime('posix')

    def runtime(self, name, exe=True):
        folder = self.root / name
        cli = folder / 'bin' / ('kicad-cli.exe' if exe else 'kicad-cli')
        cli.parent.mkdir(parents=True, exist_ok=True)
        cli.write_bytes(b'MZfixture' if exe else b'#!/bin/sh\nexit 0\n')
        (folder / 'manifest.json').write_text(json.dumps({'version': '10.99.0-bp.6'}))
        return cli

    def run_discovery(self, cli, source):
        self.env.update(TEST_KICAD_EXPECTED=str(cli), TEST_KICAD_SOURCE=source)
        result = subprocess.run([self.bun, str(self.program)], cwd=ROOT, env=self.env,
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('ok filesystem selected CLI', result.stdout)
        self.assertNotRegex(result.stdout + result.stderr, r'(?m)^(FAIL|Error:)')

    def test_windows_path_and_json_assertions(self):
        result = subprocess.run(['bend', str(ROOT / 'test/kicad_paths_test.bend')],
                                cwd=ROOT, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('ok JSON backslashes stay literal', result.stdout)
        self.assertIn('ok native Windows PATH preserves first drive', result.stdout)
        self.assertNotRegex(result.stdout + result.stderr, r'(?m)^(FAIL|Error:)')

    def test_standard_program_files_without_path_entry(self):
        self.runtime('Program Files/KiCad/9.0')
        cli = self.runtime('Program Files/KiCad/10.0')
        self.run_discovery(cli, 'system')

    def test_bundled_exe_wins_over_system(self):
        self.runtime('Program Files/KiCad/10.0')
        self.run_discovery(self.runtime('app/kicad'), 'bundled')

    def test_older_managed_exe_is_discovered(self):
        self.run_discovery(self.runtime('home/kicad/10.99.0-bp.5'), 'installed')

    def test_explicit_root_with_spaces_and_trailing_slash(self):
        cli = self.runtime('new tools/test')
        self.env['BACKPLANE_KICAD'] = str(cli.parent.parent) + '/'
        self.run_discovery(cli, 'env')

    def test_explicit_missing_root_stops_fallback(self):
        self.runtime('Program Files/KiCad/10.0')
        self.env['BACKPLANE_KICAD'] = str(self.root / 'absent')
        self.run_discovery(self.root / 'absent/bin/kicad-cli', 'env')

    def test_selected_managed_version(self):
        self.runtime('home/kicad/10.99.0-bp.5')
        self.run_discovery(self.runtime('home/kicad/10.99.0-bp.6'), 'installed')


if __name__ == '__main__':
    unittest.main(verbosity=2)
