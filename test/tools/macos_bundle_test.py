#!/usr/bin/env python3
"""Offline signing-plan tests and native codesign regression on disposable apps."""
import importlib.util
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('macos_bundle', ROOT / 'desktop/macos-bundle.py')
mac = importlib.util.module_from_spec(spec)
sys.dont_write_bytecode = True
spec.loader.exec_module(mac)


class BundleTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.app = self.root / 'Backplane 日本語.app'
        self.binary(self.app, 'Backplane')
        self.helper = self.app / 'Contents/Frameworks/Helper.app'
        self.binary(self.helper, 'Helper')
        self.service = self.app / 'Contents/Resources/backplane-service/backplane-serve'
        self.service.parent.mkdir(parents=True)
        self.service.write_bytes(bytes.fromhex('cffaedfe') + b'fixture')
        self.service.chmod(0o755)
        # Broken and escaping symlinks are never traversed or signed.
        (self.app / 'Contents/Frameworks/alias').symlink_to(self.root)

    def binary(self, app, name):
        import plistlib
        (app / 'Contents/MacOS').mkdir(parents=True)
        (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({
            'CFBundleExecutable': name, 'CFBundleIdentifier': 'dev.backplane.' + name,
            'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1'}))
        (app / 'Contents/MacOS' / name).write_bytes(bytes.fromhex('cffaedfe') + b'fixture')

    def test_inside_out_and_immutable_service(self):
        calls = []
        with patch.object(mac.sys, 'platform', 'darwin'), patch.object(mac, 'run', side_effect=lambda *a: calls.append(a)):
            result = mac.bundle(self.app, seal=True)
        self.assertTrue(result['nestedCodeVerified'])
        self.assertEqual(result['machOFiles'], 3)
        signs = [c for c in calls if '--sign' in c]
        self.assertEqual([c[-1] for c in signs[-2:]], [str(self.helper), str(self.app)])
        self.assertFalse(any(c[-1] == str(self.service) for c in signs))
        self.assertTrue(all('--deep' not in c for c in signs))
        self.assertIn(('/usr/bin/codesign', '--verify', '--strict', str(self.service)), calls)

    def test_verification_failure_propagates(self):
        with patch.object(mac.sys, 'platform', 'darwin'), patch.object(mac, 'run', side_effect=subprocess.CalledProcessError(1, 'codesign')):
            with self.assertRaises(subprocess.CalledProcessError):
                mac.bundle(self.app)

    def test_refuses_non_native_verification(self):
        with patch.object(mac.sys, 'platform', 'linux'):
            with self.assertRaisesRegex(ValueError, 'native Mac'):
                mac.bundle(self.app)

    @unittest.skipUnless(sys.platform == 'darwin', 'real macOS codesign')
    def test_native_seal_roundtrip_and_tamper_detection(self):
        # Real Apple-signed Mach-O code, no compiler or payload execution.
        (self.app / 'Contents/Frameworks/alias').unlink()
        for dest in [self.app / 'Contents/MacOS/Backplane', self.helper / 'Contents/MacOS/Helper', self.service]:
            shutil.copyfile('/usr/bin/true', dest)
            dest.chmod(0o755)
        resource = self.app / 'Contents/Resources/current.txt'
        resource.write_text('sealed resources\n')
        service_hash = hashlib.sha256(self.service.read_bytes()).hexdigest()
        subprocess.run(['/usr/bin/xattr', '-w', 'com.apple.quarantine',
                        '0083;00000000;BackplaneTest;', str(self.app)], check=True)
        mac.bundle(self.app, seal=True)
        self.assertEqual(hashlib.sha256(self.service.read_bytes()).hexdigest(), service_hash)
        quarantined = subprocess.run(['/usr/bin/xattr', '-p', 'com.apple.quarantine', str(self.app)],
                                     capture_output=True)
        self.assertNotEqual(quarantined.returncode, 0, 'Bundle retained quarantine')
        archive = self.root / 'app.tar.gz'
        with tarfile.open(archive, 'w:gz') as t:
            t.add(self.app, arcname=self.app.name)
        extracted = self.root / 'extracted'
        extracted.mkdir()
        with tarfile.open(archive) as t:
            t.extractall(extracted)
        restored = extracted / self.app.name
        mac.bundle(restored)
        (restored / 'Contents/Resources/current.txt').write_text('tampered\n')
        with self.assertRaises(subprocess.CalledProcessError):
            mac.bundle(restored)
        self.assertTrue(mac.bundle(self.app)['sealed'])
        # Both the main bundle and a nested helper's resource seal are mandatory.
        shutil.rmtree(self.helper / 'Contents/_CodeSignature')
        with self.assertRaises(subprocess.CalledProcessError):
            mac.bundle(self.app)


if __name__ == '__main__':
    unittest.main()
