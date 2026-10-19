"""Exercise the package ABI gate with actual host ELF files."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("linux_abi", ROOT / "scripts/linux-abi.py")
ABI = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ABI)


class LinuxAbiTest(unittest.TestCase):
    def test_helper_and_library_are_included(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "web").mkdir()
            (root / "web/index.html").write_text("hello")
            # No ABI on the launcher; a nested helper must still set the floor.
            (root / "backplane").write_text("#!/bin/sh\nexit 0\n")
            shutil.copyfile("/bin/ls", root / "web/helper.so")
            required = ABI.required_glibc(root)
            self.assertGreaterEqual(required, (2, 2))
            rejected = subprocess.run(["python3", str(ROOT / "scripts/linux-abi.py"),
                                       str(root), "--max", "2.0"], capture_output=True)
            self.assertEqual(rejected.returncode, 1)
            accepted = subprocess.run(["python3", str(ROOT / "scripts/linux-abi.py"),
                                       str(root), "--max", "999.0"], capture_output=True)
            self.assertEqual(accepted.returncode, 0)
            self.assertEqual(accepted.stdout.decode().strip(), ".".join(map(str, required)))

    def test_empty_package_fails(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "no versioned glibc"):
                ABI.required_glibc(folder)


if __name__ == "__main__":
    unittest.main()
