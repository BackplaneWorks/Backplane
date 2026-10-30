#!/usr/bin/env python3
"""scripts/check-helpers.py over packaged desktop apps (issue #510): synthetic
headers in .tar.gz, .zip and folders, never executed."""
import importlib.util
import io
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
import zipfile

SOURCE = Path(__file__).resolve().parents[2] / "scripts/check-helpers.py"
spec = importlib.util.spec_from_file_location("check_helpers", SOURCE)
C = importlib.util.module_from_spec(spec)
spec.loader.exec_module(C)
S = C.S

SERVICE = {"darwin-arm64": "backplane-darwin-arm64/Backplane.app/Contents/Resources/backplane-service",
           "linux-x64": "backplane-linux-x64/runtime/resources/backplane-service",
           "windows-x64": "backplane-windows-x64/resources/backplane-service"}


def header(target, signed=True):
    if target.startswith("linux-"):
        b = bytearray(32)
        b[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", b, 18, 62 if target == "linux-x64" else 183)
        return bytes(b)
    if target.startswith("darwin-"):
        cmds = struct.pack("<II", 0x19, 8) + (struct.pack("<II", 0x1d, 16) + bytes(8) if signed else b"")
        cpu = 0x1000007 if target == "darwin-x64" else 0x100000c
        return struct.pack("<IIIIIIII", 0xfeedfacf, cpu, 0, 2, 2 if signed else 1, len(cmds), 0, 0) + cmds
    b = bytearray(0x100)
    b[:2] = b"MZ"
    struct.pack_into("<I", b, 0x3c, 0x80)
    b[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<H", b, 0x84, 0x8664)
    return bytes(b)


class CheckHelpers(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def members(self, target, drop=(), mode=0o755, signed=True):
        service = S.service_name(target)
        files = {SERVICE[target] + "/" + service: (header(target), 0o755),
                 SERVICE[target] + "/BUILD.json": (b"{}", 0o644)}
        for name in S.helper_names(target):
            if name not in drop:
                files[SERVICE[target] + "/" + name] = (header(target, signed), mode)
        return files

    def tar(self, target, **kw):
        path = self.root / (target + ".tar.gz")
        with tarfile.open(path, "w:gz") as tar:
            for name, (data, mode) in self.members(target, **kw).items():
                info = tarfile.TarInfo(name)
                info.size, info.mode = len(data), mode
                tar.addfile(info, io.BytesIO(data))
        return path

    def zip(self, target, **kw):
        path = self.root / (target + ".zip")
        with zipfile.ZipFile(path, "w") as z:
            for name, (data, mode) in self.members(target, **kw).items():
                info = zipfile.ZipInfo(name)
                info.external_attr = mode << 16
                z.writestr(info, data)
        return path

    def folder(self, target, **kw):
        base = Path(tempfile.mkdtemp(prefix="dir-" + target + "-", dir=self.root))
        for name, (data, mode) in self.members(target, **kw).items():
            p = base / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)
            p.chmod(mode)
        return base

    def test_complete_apps_pass_on_every_target(self):
        for target in ("darwin-arm64", "linux-x64"):
            found = C.check(self.tar(target), target)
            self.assertEqual(sorted(found[SERVICE[target]]), sorted(S.HELPERS))
            C.check(self.folder(target), target)
            C.check(self.folder(target) / SERVICE[target], target, service_dir=True)
        found = C.check(self.zip("windows-x64"), "windows-x64")
        self.assertEqual(sorted(found[SERVICE["windows-x64"]]), sorted(S.WINDOWS_HELPERS))

    def test_the_0_13_0_mac_app_is_refused(self):
        # the service and nothing beside it, as v0.13.0 shipped
        with self.assertRaisesRegex(ValueError, "Missing helper backplane-kipc"):
            C.check(self.tar("darwin-arm64", drop=S.HELPERS), "darwin-arm64")

    def test_each_missing_helper_is_refused(self):
        for name in S.HELPERS:
            for make in (self.tar, self.folder):
                with self.subTest(name=name, make=make.__name__), self.assertRaisesRegex(ValueError, "Missing helper " + name):
                    C.check(make("linux-x64", drop=(name,)), "linux-x64")
        with self.assertRaisesRegex(ValueError, "backplane-step2glb.exe"):
            C.check(self.zip("windows-x64", drop=("backplane-step2glb.exe",)), "windows-x64")

    def test_mode_arch_and_signature(self):
        with self.assertRaisesRegex(ValueError, "not executable"):
            C.check(self.tar("linux-x64", mode=0o644), "linux-x64")
        with self.assertRaisesRegex(ValueError, "not a linux-arm64 executable"):
            C.check(self.tar("linux-x64"), "linux-arm64")
        with self.assertRaisesRegex(ValueError, "not code signed"):
            C.check(self.tar("darwin-arm64", signed=False), "darwin-arm64")

    def test_an_archive_without_a_service_is_refused(self):
        path = self.root / "empty.tar.gz"
        with tarfile.open(path, "w:gz"):
            pass
        with self.assertRaisesRegex(ValueError, "No backplane-service"):
            C.check(path, "linux-x64")


if __name__ == "__main__":
    unittest.main(verbosity=2)
