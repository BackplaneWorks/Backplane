import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "android_smoke", Path(__file__).resolve().parents[2] / "scripts/android-existing-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class ExistingArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = "1" * 40
        self.apk = self.root / "existing.apk"
        self.apk.write_bytes(b"existing signed artifact stand-in")
        self.receipt = dict(status="ARTIFACTS_BUILT_PENDING_TARGET_ACCEPTANCE",
                            sourceCommit=self.source, version="0.14.0", channel="release",
                            package=smoke.PACKAGE, version_name="0.14.0",
                            version_code=2026110302, debuggable=False,
                            artifacts={"apk": {"path": "existing.apk", "sha256":
                                hashlib.sha256(self.apk.read_bytes()).hexdigest()}})
        self.save()

    def save(self):
        (self.root / "receipt.json").write_text(json.dumps(self.receipt))

    def bind(self):
        return smoke.bound_apk(self.root, self.source, "0.14.0", "2026110302")

    def test_accepts_matching_existing_artifact(self):
        self.assertEqual(self.bind()[0], self.apk)

    def test_accepts_actual_configured_artifact_layout(self):
        downloaded = self.root / "downloaded"
        nested = downloaded / "bootstrap-android"
        nested.mkdir(parents=True)
        shutil.copy2(self.apk, nested / self.apk.name)
        shutil.copy2(self.root / "receipt.json", nested / "receipt.json")
        apk, receipt = smoke.bound_apk(smoke.producer_input(downloaded),
                                       self.source, "0.14.0", "2026110302")
        self.assertEqual(apk, nested / self.apk.name)
        self.assertEqual(receipt, self.receipt)

    def test_rejects_flat_and_linked_download_layout(self):
        with self.assertRaises(ValueError):
            smoke.producer_input(self.root)
        downloaded = self.root / "downloaded"
        downloaded.mkdir()
        (downloaded / "bootstrap-android").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            smoke.producer_input(downloaded)

    def test_rejects_other_source_channel_version_and_package(self):
        for field, bad in [("sourceCommit", "2" * 40), ("channel", "dev"),
                           ("version_code", 2026110301), ("package", "other.app")]:
            with self.subTest(field=field):
                original = self.receipt[field]
                self.receipt[field] = bad
                self.save()
                with self.assertRaises(ValueError):
                    self.bind()
                self.receipt[field] = original

    def test_rejects_changed_artifact(self):
        self.apk.write_bytes(b"different artifact")
        with self.assertRaises(ValueError):
            self.bind()

    def test_rejects_indirect_member(self):
        other = self.root / "other.apk"
        self.apk.rename(other)
        self.apk.symlink_to(other)
        with self.assertRaises(ValueError):
            self.bind()


if __name__ == "__main__":
    unittest.main()
