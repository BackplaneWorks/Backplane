"""Check sandbox CLI argv and exit handling without requiring a desktop."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/kicad-flatpak.sh"


class KiCadFlatpakTest(unittest.TestCase):
    def invoke(self, sandbox=False, ref=None, code=0):
        with tempfile.TemporaryDirectory(prefix="bp flatpak ") as folder:
            root = Path(folder)
            recorder = root / "recorder"
            recorder.write_text("#!/usr/bin/python3\nimport json, os, sys\n"
                                "print(json.dumps(sys.argv[1:]))\n"
                                "sys.exit(int(os.environ['TEST_EXIT']))\n")
            recorder.chmod(0o755)
            for name in ("flatpak", "flatpak-spawn"):
                (root / name).symlink_to(recorder)
            env = dict(os.environ, PATH=f"{root}:/usr/bin:/bin", TEST_EXIT=str(code))
            env.pop("FLATPAK_ID", None)
            env.pop("BACKPLANE_KICAD_FLATPAK_REF", None)
            if sandbox:
                env["FLATPAK_ID"] = "io.github.i2cjak.Backplane"
            if ref is not None:
                env["BACKPLANE_KICAD_FLATPAK_REF"] = ref
            args = ["pcb", "export", "svg", "-o", f"{root}/out dir/",
                    f"{root}/a;$(touch nope).kicad_pcb"]
            result = subprocess.run([str(SCRIPT), *args], env=env, cwd=root,
                                    capture_output=True, text=True)
            self.assertFalse((root / "nope").exists())
            return result, args, str(root)

    def test_native_argv(self):
        result, args, _ = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertEqual(argv, ["run", "--filesystem=host", "--filesystem=/tmp",
                               "--filesystem=/var/tmp", "--command=kicad-cli",
                               "org.kicad.KiCad", *args])

    def test_nested_sandbox_and_branch(self):
        result, args, cwd = self.invoke(True, "org.kicad.KiCad//stable")
        argv = json.loads(result.stdout)
        self.assertEqual(argv[:5], ["--host", "--watch-bus", f"--directory={cwd}",
                                   "flatpak", "run"])
        self.assertEqual(argv[-len(args)-1:], ["org.kicad.KiCad//stable", *args])

    def test_exit_status(self):
        result, _, _ = self.invoke(code=42)
        self.assertEqual(result.returncode, 42)

    def test_option_as_ref_rejected(self):
        result, _, _ = self.invoke(ref="--command=sh")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
