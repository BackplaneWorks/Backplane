"""Run the production launcher with synthetic apps and command stand-ins."""
import ast
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class KiCadOpenTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="bp-kicad-open-")
        self.addCleanup(self.tmp.cleanup)
        self.home = Path(self.tmp.name)
        self.bin = self.home / "commands"
        self.bin.mkdir()
        self.runtime = self.home / "runtime with spaces"
        self.runtime.mkdir()
        self.record = self.home / "args.json"
        text = (ROOT / "src/server/kicad.bend").read_text()
        self.script = ast.literal_eval(re.search(r'def KiCad.open.script\(\) -> String:\n  ("[^\n]+")', text)[1])
        self.command("uname", "#!/bin/sh\nprintf '%s\\n' \"$TEST_PLATFORM\"\n")
        for name in ["open", "setsid"]:
            self.command(name, "#!/usr/bin/env python3\nimport json,os,sys\nopen(os.environ['TEST_RECORD'],'w').write(json.dumps(sys.argv[1:]))\n")

    def command(self, name, text):
        path = self.bin / name
        path.write_text(text)
        path.chmod(0o755)

    def run_launcher(self, platform, files):
        return subprocess.run(["sh", "-c", self.script, "sh", str(self.runtime), *files],
                              env={**os.environ, "PATH": str(self.bin) + os.pathsep + os.environ["PATH"],
                                   "TEST_PLATFORM": platform, "TEST_RECORD": str(self.record)},
                              text=True, capture_output=True, timeout=5)

    def test_mac_routes_project_and_lone_editors(self):
        for files, app, target in [(["project with spaces.kicad_pro", "b", "s"], "KiCad.app", "project with spaces.kicad_pro"),
                                   (["", "board.kicad_pcb", "s"], "KiCad.app/Contents/Applications/pcbnew.app", "board.kicad_pcb"),
                                   (["", "", "sheet.kicad_sch"], "KiCad.app/Contents/Applications/eeschema.app", "sheet.kicad_sch")]:
            with self.subTest(app=app):
                (self.runtime / app).mkdir(parents=True, exist_ok=True)
                result = self.run_launcher("Darwin", files)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(self.record.read_text()), ["-a", str(self.runtime / app), target])

    def test_missing_package_and_project_refuse(self):
        for files, diagnostic in [(["p.kicad_pro", "", ""], "Backplane KiCad has no kicad"),
                                  (["", "", ""], "no KiCad project here")]:
            result = self.run_launcher("Darwin", files)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(diagnostic, result.stdout)
            self.assertFalse(self.record.exists())

    def test_other_platform_launchers_and_exe_fallback(self):
        (self.runtime / "bin").mkdir()
        for platform, binary in [("Linux", "kicad"), ("MINGW64_NT", "kicad.exe")]:
            with self.subTest(platform=platform):
                path = self.runtime / "bin" / binary
                path.write_text("#!/bin/sh\nexit 0\n")
                path.chmod(0o755)
                result = self.run_launcher(platform, ["p.kicad_pro", "", ""])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(self.record.read_text()), ["-f", str(path), "p.kicad_pro"])
                path.unlink()


if __name__ == "__main__":
    unittest.main()
