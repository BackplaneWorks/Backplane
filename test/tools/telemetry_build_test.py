#!/usr/bin/env python3
"""Exercise release baking and fail-closed developer builds without real keys."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
KEY = "ab" * 32


class TelemetryBuild(unittest.TestCase):
    def run_header(self, **settings):
        with tempfile.TemporaryDirectory(prefix="telemetry build ") as folder:
            header = Path(folder) / "compile only header.h"
            env = {k: v for k, v in os.environ.items() if not k.startswith("BACKPLANE_TELEMETRY_")}
            env.pop("BACKPLANE_VERSION_STAMP", None)
            env.pop("BACKPLANE_VERSION", None)
            env.update(settings)
            result = subprocess.run([sys.executable, str(ROOT / "scripts/telemetry-header.py"), str(header)],
                                    env=env, capture_output=True, text=True)
            self.assertNotIn(KEY, result.stdout + result.stderr)
            text = header.read_text() if header.exists() else ""
            if header.exists():
                self.assertEqual(header.stat().st_mode & 0o777, 0o600)
            return result.returncode, text

    def release(self, **changes):
        env = dict(BACKPLANE_TELEMETRY_BAKE="1", BACKPLANE_VERSION="v0.12.0",
                   BACKPLANE_TELEMETRY_URL="https://backplane.works/api/v1/events",
                   BACKPLANE_TELEMETRY_KID="rel-test", BACKPLANE_TELEMETRY_KEY=KEY)
        env.update(changes)
        return self.run_header(**env)

    def test_dev_environment_stays_quiet(self):
        rc, text = self.run_header(BACKPLANE_TELEMETRY_KEY=KEY,
                                   BACKPLANE_TELEMETRY_URL="https://backplane.works/api/v1/events")
        self.assertEqual(rc, 0)
        self.assertNotIn(KEY, text)
        self.assertIn('BP_TELEMETRY_CHANNEL ""', text)

    def test_stable_and_nightly_bind_package_channel(self):
        for version, channel in [("v0.12.0", "release"), ("v0.12.1-nightly.20261019.1", "nightly")]:
            rc, text = self.release(BACKPLANE_VERSION=version)
            self.assertEqual(rc, 0)
            self.assertIn(f'BP_TELEMETRY_CHANNEL "{channel}"', text)
            self.assertIn(KEY, text)

    def test_mismatch_and_developer_version_refused(self):
        for changes in [dict(BACKPLANE_TELEMETRY_CHANNEL="nightly"),
                        dict(BACKPLANE_VERSION_STAMP="0.12.1"),
                        dict(BACKPLANE_VERSION="v0.12.0-dev.20261019.1")]:
            rc, text = self.release(**changes)
            self.assertNotEqual(rc, 0)
            self.assertEqual(text, "")

    def test_missing_configuration_is_build_kill_switch(self):
        for field in ("URL", "KID", "KEY"):
            rc, text = self.release(**{"BACKPLANE_TELEMETRY_" + field: ""})
            self.assertEqual(rc, 0)
            self.assertNotIn(KEY, text)
            self.assertNotIn("backplane.works", text)

    def test_invalid_configuration_never_reaches_header(self):
        for field, value in [("URL", "https://elsewhere.invalid/api/v1/events"),
                             ("KID", 'rel-test"\n#error injected'), ("KEY", "ab" * 31)]:
            rc, text = self.release(**{"BACKPLANE_TELEMETRY_" + field: value})
            self.assertNotEqual(rc, 0)
            self.assertEqual(text, "")


if __name__ == "__main__":
    unittest.main()
