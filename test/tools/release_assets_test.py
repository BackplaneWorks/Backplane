"""Fail-closed metadata/immutability tests; no product build or publication."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_assets", ROOT / "scripts/release-assets.py")
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)
SHA = "a" * 40
TAG = "v0.11.2"


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.out = self.root / "out"
        self.out.mkdir()
        self.templates = self.root / "templates"
        (self.templates / "aur").mkdir(parents=True)
        (self.templates / "install.sh.in").write_text('@VERSION@ @SHA_LINUX_X64@ @SHA_LINUX_ARM64@ @SHA_DARWIN_ARM64@\nname="backplane-$VER-$os-$arch.tar.gz"\nurl="https://github.com/$REPO/releases/download/$VER/$name"\n')
        (self.templates / "aur/PKGBUILD.in").write_text('@PKGVER@ @SHA_LINUX_X64@ @SHA_LINUX_ARM64@\nsource_x86_64=("$url/releases/download/v$pkgver/backplane-v$pkgver-linux-x64.tar.gz")\n')
        self.payload(TAG)

    def payload(self, tag, source=SHA, package_version=None, channel=None):
        for p in self.out.iterdir():
            p.unlink()
        version, derived = assets.identity(source, tag)
        names = [f"backplane-{tag}-{plat}.tar.gz" for plat in ["linux-x64", "linux-arm64", "darwin-arm64"]]
        names += [f"backplane-{tag}-{plat}.AppImage" for plat in ["linux-x64", "linux-arm64"]]
        names += [f"backplane_{version.replace('-', '~')}_{arch}.deb" for arch in ["amd64", "arm64"]]
        for name in names:
            (self.out / name).write_bytes(b"fixture candidate bytes")
        z = self.out / f"backplane-{tag}-windows-x64.zip"
        with zipfile.ZipFile(z, "w") as archive:
            archive.writestr("backplane-windows-x64/BUILD.json", json.dumps({"commit": source, "version": package_version or tag, "target": "windows-x64", "telemetryChannel": channel or derived}))
            archive.writestr("backplane-windows-x64/SOURCE.json", json.dumps({"commit": source, "version": tag, "telemetry_channel": channel or derived}))
        z.with_suffix(".zip.sha256").write_text(f"{assets.digest(z)}  {z.name}\n")

    def collect(self, tag=TAG):
        assets.collect(self.out, SHA, tag, self.templates)

    def test_release_bake_fails_closed_without_printing_values(self):
        env = {"BACKPLANE_GOOGLE_CLIENT_ID": "fixture-client-id",
               "BACKPLANE_GOOGLE_CLIENT_SECRET": "fixture-client-secret",
               "BACKPLANE_TELEMETRY_BAKE": "1",
               "BACKPLANE_TELEMETRY_URL": "https://backplane.works/api/v1/events",
               "BACKPLANE_TELEMETRY_KID": "rel-1", "BACKPLANE_TELEMETRY_KEY": "a" * 64}
        assets.check_bake(env, "release")
        for name in env:
            broken = dict(env)
            broken.pop(name)
            with self.assertRaises(ValueError) as result:
                assets.check_bake(broken, "release")
            self.assertNotIn("a" * 64, str(result.exception))
            self.assertNotIn("fixture-client-secret", str(result.exception))
        for name, value in [("BACKPLANE_GOOGLE_BAKE", "0"),
                            ("BACKPLANE_GOOGLE_BAKE", "false"),
                            ("BACKPLANE_GOOGLE_CLIENT_ID", " "),
                            ("BACKPLANE_GOOGLE_CLIENT_SECRET", " "),
                            ("BACKPLANE_TELEMETRY_KEY", "not hex"),
                            ("BACKPLANE_TELEMETRY_KID", "dev-1"),
                            ("BACKPLANE_TELEMETRY_CHANNEL", "nightly"),
                            ("BACKPLANE_TELEMETRY_URL", "http://loopback.invalid")]:
            with self.assertRaises(ValueError):
                assets.check_bake(dict(env, **{name: value}), "release")

    def test_explicit_compile_endpoint_reaches_each_native_path(self):
        # The approved public endpoint is an input, independent of repo variables.
        for name in ("release.yml", "windows.yml"):
            text = (ROOT / ".github/workflows" / name).read_text()
            self.assertNotIn("vars.BACKPLANE_TELEMETRY_URL", text)
            self.assertEqual(text.count("default: https://backplane.works/api/v1/events"), 2)
            self.assertIn("BACKPLANE_TELEMETRY_URL: ${{ inputs.telemetry_url }}", text)
        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("telemetry_url: ${{ inputs.telemetry_url }}", release)
        nightly = (ROOT / ".github/workflows/nightly.yml").read_text()
        self.assertIn("telemetry_url: https://backplane.works/api/v1/events", nightly)

    def test_composed_workflow_retains_cef_gate_and_security_payload(self):
        text = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("BACKPLANE_CEF_RELEASE: ${{ matrix.target == 'linux-x64' && '1' || '0' }}", text)
        self.assertIn("bun tools/browser/test/popup-cef.ts", text)
        self.assertIn("            dist/kistack\n", text)
        self.assertEqual(text.count('--tag "$RELEASE_TAG" --require-cef'), 2)

    def test_stable_and_full_nightly_channel(self):
        self.collect()
        assets.verify(self.out, SHA, TAG)
        self.assertEqual(json.loads((self.out / "release-assets.json").read_text())["channel"], "release")
        nightly = "v0.11.2-nightly.20261019.1"
        self.payload(nightly)
        self.collect(nightly)
        assets.verify(self.out, SHA, nightly)
        self.assertFalse((self.out / "PKGBUILD").exists())
        self.assertEqual(json.loads((self.out / "release-assets.json").read_text())["channel"], "nightly")

    def test_wrong_frozen_source_or_version_rejected(self):
        self.collect()
        for source, tag in [("b" * 40, TAG), (SHA, "v0.11.3")]:
            with self.assertRaises(ValueError):
                assets.verify(self.out, source, tag)
        for ref in ["main", "a" * 39, "A" * 40]:
            with self.assertRaises(ValueError):
                assets.identity(ref, TAG)

    def test_native_version_limits_match_bake_generator(self):
        # Existing run identity plus attempt must fit the existing product contract.
        n = 38623094261 * 1000 + 1
        assets.identity(SHA, f"v0.11.2-nightly.20261019.{n}")
        for tag in ["0.11.2", "v10000.1.2", "v0.11.2-nightly.20261019." + "1" * 16]:
            with self.assertRaises(ValueError):
                assets.identity(SHA, tag)

    def test_wrong_build_run_rejected(self):
        self.collect()
        with self.assertRaises(ValueError):
            assets.verify(self.out, SHA, TAG, run_id="wrong-run")

    def test_wrong_windows_stamp_or_channel_rejected(self):
        for version, channel in [("v0.11.1", "release"), (TAG, "nightly")]:
            self.payload(TAG, package_version=version, channel=channel)
            with self.assertRaises(ValueError):
                self.collect()

    def test_mutated_missing_extra_and_symlink_assets_rejected(self):
        self.collect()
        p = self.out / f"backplane-{TAG}-linux-arm64.tar.gz"
        p.write_bytes(b"changed")
        with self.assertRaises(ValueError):
            assets.verify(self.out, SHA, TAG)
        self.payload(TAG)
        p.unlink()
        with self.assertRaises(ValueError):
            self.collect()
        self.payload(TAG)
        (self.out / "private-key").write_text("fixture, not a key")
        with self.assertRaises(ValueError):
            self.collect()
        self.payload(TAG)
        p.unlink()
        p.symlink_to(self.out / f"backplane-{TAG}-darwin-arm64.tar.gz")
        with self.assertRaises(ValueError):
            self.collect()

    def test_cef_requirement_and_ambiguous_archive(self):
        self.collect()
        with self.assertRaises(ValueError):
            assets.verify(self.out, SHA, TAG, require_cef=True)
        self.payload(TAG)
        x = self.out / f"backplane-{TAG}-linux-x64.tar.gz"
        cef = self.out / f"backplane-{TAG}-linux-x64-cef.tar.gz"
        cef.write_bytes(x.read_bytes())
        with self.assertRaises(ValueError):
            self.collect()
        x.unlink()
        with self.assertRaises(ValueError):
            self.collect()  # Existing installer template does not handle the CEF name.

    def test_matched_cef_archive_can_be_verified(self):
        old = self.out / f"backplane-{TAG}-linux-x64.tar.gz"
        old.rename(self.out / f"backplane-{TAG}-linux-x64-cef.tar.gz")
        installer = self.templates / "install.sh.in"
        recipe = self.templates / "aur/PKGBUILD.in"
        for template in (installer, recipe):
            template.write_text(template.read_text() + "# unrelated-cef comment\n")
        with self.assertRaises(ValueError):
            self.collect()  # A comment cannot change the download target.
        installer.write_text(installer.read_text().replace('name="backplane-$VER-$os-$arch.tar.gz"',
            'archive_plat=$os-$arch\n[ "$archive_plat" != linux-x64 ] || archive_plat=linux-x64-cef\nname="backplane-$VER-$archive_plat.tar.gz"'))
        with self.assertRaises(ValueError):
            self.collect()  # Both installer and recipe must select the CEF archive.
        recipe.write_text(recipe.read_text().replace('linux-x64.tar.gz', 'linux-x64-cef.tar.gz'))
        assets.collect(self.out, SHA, TAG, self.templates, require_cef=True)
        assets.verify(self.out, SHA, TAG, require_cef=True)

    def test_checksums_and_inventory_cannot_omit_files(self):
        self.collect()
        sums = self.out / "SHA256SUMS"
        sums.write_text("\n".join(sums.read_text().splitlines()[1:]) + "\n")
        with self.assertRaises(ValueError):
            assets.verify(self.out, SHA, TAG)
        self.payload(TAG)
        self.collect()
        manifest = self.out / "release-assets.json"
        d = json.loads(manifest.read_text())
        d["channel"] = "nightly"
        manifest.write_text(json.dumps(d))
        with self.assertRaises(ValueError):
            assets.verify(self.out, SHA, TAG)


if __name__ == "__main__":
    unittest.main()
