"""Compile/run the actual Rt.save boundary on Boat, or a hosted Windows runner."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Cache(unittest.TestCase):
    def test_production_save_literal_windows_paths(self):
        with tempfile.TemporaryDirectory(prefix='bp-cache 日本 ') as tmp:
            folder = Path(tmp)
            program = folder / 'cache.js'
            subprocess.run(['bend', 'test/native/kicad_cache.bend', '-o', str(program)],
                           cwd=ROOT, check=True, timeout=90)
            home = folder / 'home with spaces'
            home.mkdir()
            values = ['C:\\backplane_kicad\\', 'C:\\bp927\\new tools\\test\\',
                      'D:\\Program Files\\KiCad\\', '\\\\server\\share\\new\\test\\',
                      'quotes " and actual controls \b\n\t 日本']
            for setting in values:
                with self.subTest(setting=setting):
                    env = dict(os.environ, TEST_KICAD_CACHE_HOME=str(home),
                               TEST_KICAD_CACHE_SETTING=setting)
                    result = subprocess.run([shutil.which('bun'), str(program)],
                                            cwd=ROOT, env=env, text=True,
                                            capture_output=True, timeout=30)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertNotRegex(result.stdout + result.stderr, r'(?m)^(FAIL|Error:)')
                    self.assertIn('ok native saved Windows setting', result.stdout)
                    self.assertEqual(json.loads((home / 'kicad-health.json').read_text())['setting'], setting)
                    self.assertEqual([p.name for p in home.iterdir()], ['kicad-health.json'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
