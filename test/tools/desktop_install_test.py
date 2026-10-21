#!/usr/bin/env python3
"""Exercise the real installer and Linux desktop launcher in disposable homes."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(sys.platform.startswith("linux"), "Linux desktop integration")
class DesktopInstallTest(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="bp-desktop-install-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        package = self.root / "package" / "backplane-linux-x64"
        package.mkdir(parents=True)
        (package / "web").mkdir()
        (package / "web" / "index.html").write_text("fixture\n")
        executable = '#!/bin/sh\nprintf "%s\\n" "$0" "$@" > "$DESKTOP_LAUNCH_RECORD"\n'
        # backplane is the launcher (deploy/backplane-launch.sh); a stub stands in
        for name in ["backplane", "backplane-serve"]:
            path = package / name
            path.write_text(executable)
            path.chmod(0o755)
        shutil.copy2(ROOT / "scripts/install-release.sh", package)
        shutil.copy2(ROOT / "deploy/backplane.desktop", package)
        (package / "icon").mkdir()
        for name in ["backplane-desktop.svg", "backplane-desktop-256.png"]:
            shutil.copy2(ROOT / "assets/icon" / name, package / "icon" / name)
        self.archive = self.root / "release.tar.gz"
        with tarfile.open(self.archive, "w:gz") as archive:
            archive.add(package, arcname=package.name)
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        script = (ROOT / "deploy/install.sh.in").read_text().replace("@VERSION@", "v0.0.0-test")
        for platform in ["LINUX_X64", "LINUX_ARM64", "DARWIN_ARM64"]:
            script = script.replace("@SHA_" + platform + "@", digest)
            # the stub curl serves this one archive as the desktop app too
            script = script.replace("@DESKTOP_SHA_" + platform + "@", digest)
        self.installer = self.root / "install.sh"
        self.installer.write_text(script)
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.tool("uname", 'case "$1" in -s) printf "%s\\n" "$TEST_OS";; -m) echo x86_64;; esac')
        self.tool("curl", 'while [ "$#" -gt 0 ]; do if [ "$1" = -o ]; then cp "$TEST_ARCHIVE" "$2"; exit; fi; shift; done; exit 1')
        # Cache refresh is optional; failure must not undo an otherwise good install.
        self.tool("update-desktop-database", "exit 1")
        self.tool("gtk-update-icon-cache", "exit 1")
        self.env = {**os.environ, "HOME": str(self.root / "home"),
                    "PATH": str(self.tools) + os.pathsep + os.environ["PATH"],
                    "TEST_OS": "Linux", "TEST_ARCHIVE": str(self.archive),
                    "BACKPLANE_PREFIX": str(self.root / "installation"),
                    "BACKPLANE_BIN": str(self.root / "bin"),
                    "XDG_DATA_HOME": str(self.root / "desktop data"),
                    "DESKTOP_LAUNCH_RECORD": str(self.root / "launched.txt")}
        self.env.pop("BACKPLANE_NO_DESKTOP", None)
        self.env.pop("DBUS_SESSION_BUS_ADDRESS", None)

    def tool(self, name, body):
        path = self.tools / name
        path.write_text("#!/bin/sh\n" + body + "\n")
        path.chmod(0o755)

    def install(self):
        result = subprocess.run(["sh", str(self.installer)], env=self.env,
                                text=True, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def data_home(self):
        return Path(self.env.get("XDG_DATA_HOME", str(Path(self.env["HOME"]) / ".local/share")))

    def desktop(self):
        return self.data_home() / "applications/backplane.desktop"

    def check_launch(self):
        launcher = shutil.which("gio")
        self.assertIsNotNone(launcher, "Linux desktop checks require gio")
        record = Path(self.env["DESKTOP_LAUNCH_RECORD"])
        record.unlink(missing_ok=True)
        validator = shutil.which("desktop-file-validate")
        if validator:
            subprocess.run([validator, str(self.desktop())], check=True, capture_output=True)
        result = subprocess.run([launcher, "launch", str(self.desktop())], env=self.env,
                                text=True, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(record.exists(), "desktop launcher did not execute the fixture")
        bindir = Path(self.env.get("BACKPLANE_BIN", str(Path(self.env["HOME"]) / ".local/bin")))
        self.assertEqual(record.read_bytes(), (str(bindir / "backplane") + "\n").encode())

    def test_installs_and_launches_default_entry(self):
        self.env.pop("XDG_DATA_HOME")
        self.env.pop("BACKPLANE_BIN")
        self.env.pop("BACKPLANE_PREFIX")
        result = self.install()
        self.assertIn("applications menu", result.stdout)
        self.assertIn("Icon=backplane-desktop\n", self.desktop().read_text())
        for relative, source in [
            ("scalable/apps/backplane-desktop.svg", "backplane-desktop.svg"),
            ("256x256/apps/backplane-desktop.png", "backplane-desktop-256.png"),
        ]:
            actual = self.data_home() / "icons/hicolor" / relative
            self.assertEqual(actual.read_bytes(), (ROOT / "assets/icon" / source).read_bytes())
        self.check_launch()

    def test_custom_paths_round_trip_through_real_launcher(self):
        for name in ["with spaces", "ampersand&pipe|", 'quote"dollar$backtick`slash\\',
                     "literal%f%u%%", "café 日本語", "tabs\tand\nnewlines\r", "equals=sign",
                     "'; exit 73; #$(touch injected)"]:
            with self.subTest(name=name):
                self.env["BACKPLANE_BIN"] = str(self.root / name / "bin")
                self.install()
                self.check_launch()
        self.assertFalse((self.root / "injected").exists())

    def test_opt_out_leaves_existing_entry_untouched(self):
        self.install()
        self.desktop().write_text("existing user entry\n")
        self.env["BACKPLANE_NO_DESKTOP"] = "1"
        result = self.install()
        self.assertEqual(self.desktop().read_text(), "existing user entry\n")
        self.assertNotIn("applications menu", result.stdout)

    def test_mac_install_does_not_add_linux_desktop_files(self):
        self.env["TEST_OS"] = "Darwin"
        # The macOS archive has the same minimal payload with its own platform name.
        with tarfile.open(self.archive, "w:gz") as archive:
            archive.add(self.root / "package/backplane-linux-x64", arcname="backplane-darwin-arm64")
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        script = self.installer.read_text()
        script = re.sub(r'^SHA_DARWIN_ARM64="[^"]*"', 'SHA_DARWIN_ARM64="' + digest + '"', script, flags=re.M)
        script = re.sub(r'^DESKTOP_SHA_DARWIN_ARM64="[^"]*"', 'DESKTOP_SHA_DARWIN_ARM64="' + digest + '"', script, flags=re.M)
        self.installer.write_text(script)
        self.tool("uname", 'case "$1" in -s) echo Darwin;; -m) echo arm64;; esac')
        self.install()
        self.assertFalse(self.desktop().exists())


if __name__ == "__main__":
    unittest.main()
