"""Validate the generated package and host-tool process boundary."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("package_flatpak", ROOT / "scripts/package-flatpak.py")
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class FlatpakPackageTest(unittest.TestCase):
    def test_manifest_sources_and_runtime(self):
        manifest = PACKAGE.manifest(Path("/package"), ROOT / "deploy/flatpak")
        self.assertEqual(manifest["runtime"], "org.freedesktop.Platform")
        self.assertIn("--talk-name=org.freedesktop.Flatpak", manifest["finish-args"])
        self.assertIn("--env=BACKPLANE_NO_UPDATE=1", manifest["finish-args"])
        for source in manifest["modules"][0]["sources"][1:]:
            self.assertTrue(Path(source["path"]).is_file(), source)
        ET.parse(ROOT / "deploy/flatpak" / f"{PACKAGE.APP_ID}.metainfo.xml")

    def test_host_tool_argv_and_exit(self):
        with tempfile.TemporaryDirectory(prefix="bp host tool ") as folder:
            root = Path(folder)
            (root / "git").symlink_to(ROOT / "deploy/flatpak/host-tool")
            spawn = root / "flatpak-spawn"
            spawn.write_text("#!/usr/bin/python3\nimport json, sys\n"
                             "print(json.dumps(sys.argv[1:]))\nsys.exit(42)\n")
            spawn.chmod(0o755)
            args = ["show", "branch name;$(touch nope):board.kicad_pcb"]
            result = subprocess.run([str(root / "git"), *args], cwd=root,
                                    env=dict(os.environ, PATH=f"{root}:/usr/bin:/bin"),
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 42)
            argv = json.loads(result.stdout)
            self.assertEqual(argv[:5], ["--host", "--watch-bus", f"--directory={root}",
                                       "/bin/bash", "-lc"])
            self.assertEqual(argv[-4:], ["bash", "git", *args])
            self.assertFalse((root / "nope").exists())

    def test_unknown_host_tool_fails(self):
        result = subprocess.run([str(ROOT / "deploy/flatpak/host-tool")], capture_output=True)
        self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
