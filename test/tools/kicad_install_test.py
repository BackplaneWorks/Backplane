#!/usr/bin/env python3
"""Exercise the exact embedded installer using tiny archives and offline curl.

Runs tar fixtures on POSIX; on Cygwin also runs actual Windows PowerShell ZIP
extraction. Nothing executes a downloaded CLI or touches the user's runtime.
"""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / 'src/core/kicad_install.bend'
SCRIPT = json.loads(re.search(r'def Install.script\(\) -> String:\s*(".*")', MODULE.read_text()).group(1))
IS_CYGWIN = sys.platform == 'cygwin'


class Installer(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='bp-kicad-install-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.parent = self.root / 'user data 日本' / 'kicad'
        self.fake = self.root / 'fake'; self.fake.mkdir()
        curl = self.fake / 'curl'
        curl.write_text('#!' + sys.executable + '\n' + '''import os,sys,shutil
from pathlib import Path
args=sys.argv[1:]
with open(os.environ['MOCK_LOG'],'a') as f:f.write(args[-1]+'\\n')
assert args[:4]==['--proto','=https','--tlsv1.2','-fsSL'],args
out=args[args.index('-o')+1]
if os.environ.get('MOCK_DOWNLOAD_FAIL'):
 Path(out).write_bytes(b'partial');sys.exit(22)
shutil.copyfile(os.environ['MOCK_SUMS' if args[-1].endswith('SHA256SUMS') else 'MOCK_ARCHIVE'],out)
''')
        curl.chmod(0o700)
        self.env = dict(os.environ, PATH=str(self.fake) + os.pathsep + os.environ['PATH'],
                        MOCK_LOG=str(self.root / 'requests'))

    def fixture(self, platform='linux-x86_64', fmt='tar.gz', change=None, member=None):
        version = '10.99.0-bp.4' if platform == 'linux-x86_64' else '10.99.0-bp.6'
        top = f'backplane-kicad-{platform}-{version}'
        cli = 'bin/kicad-cli.exe' if platform == 'windows-x86_64' else 'bin/kicad-cli'
        files = {cli: b'MZfixture' if cli.endswith('.exe') else b'#!/bin/sh\nexit 88\n',
                 'manifest.json': json.dumps({'version': version, 'platform': platform}).encode(),
                 'share/backplane/api.desc': b'fixture descriptor',
                 'licenses/NOTICE.txt': b'fixture license'}
        if change: change(files)
        asset = top + '.' + fmt
        archive = self.root / asset
        if fmt == 'tar.gz':
            with tarfile.open(archive, 'w:gz') as t:
                for name, data in files.items():
                    entry = tarfile.TarInfo(top + '/' + name)
                    entry.size = len(data); entry.mode = 0o755 if name == cli else 0o644
                    t.addfile(entry, io.BytesIO(data))
                if member:
                    entry = tarfile.TarInfo(member); entry.size = 6
                    t.addfile(entry, io.BytesIO(b'escape'))
        else:
            with zipfile.ZipFile(archive, 'w') as z:
                for name, data in files.items(): z.writestr(top + '/' + name, data)
                if member: z.writestr(member, b'escape')
        self.env['MOCK_ARCHIVE'] = str(archive)
        sums = self.root / 'SHA256SUMS'
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        sums.write_text(digest + '  ' + asset + '\n')
        self.env['MOCK_SUMS'] = str(sums)
        self.args = [str(self.parent), platform, version, 'backplane-kicad-v' + version,
                     asset, digest, fmt, cli]
        return archive

    def run_install(self, success=True, args=None, env=None):
        r = subprocess.run(['sh', '-c', SCRIPT, 'sh'] + (args or self.args),
                           env=env or self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(r.returncode == 0, success, r.stdout + r.stderr)
        # Failed staging cannot leak partial directories or stale install locks.
        if self.parent.exists():
            self.assertFalse(list(self.parent.glob('.install-*')), r.stdout + r.stderr)
        return r

    def test_tar_install_spaced_unicode_path_and_layout(self):
        self.fixture(); self.run_install()
        target = self.parent / self.args[2]
        self.assertEqual((target / self.args[-1]).read_bytes(), b'#!/bin/sh\nexit 88\n')
        self.assertTrue((target / 'licenses/NOTICE.txt').is_file())
        urls = (self.root / 'requests').read_text().splitlines()
        self.assertEqual(len(urls), 2)
        self.assertIn('/backplane-kicad-v10.99.0-bp.4/', urls[0])

    def test_tar_install_backslash_and_escape_sequence_path(self):
        self.parent = self.root / r'C:\backplane_kicad\new tools\test' / 'kicad'
        self.fixture(); self.run_install()
        self.assertTrue((self.parent / self.args[2] / 'bin/kicad-cli').is_file())

    @unittest.skipUnless(shutil.which("shasum"), "Darwin-only checksum tool absent; Windows uses sha256sum")
    def test_darwin_tar_without_gnu_sha256sum(self):
        self.fixture('darwin-arm64')
        # Minimal PATH ensures the real shasum fallback is exercised.
        tools = self.root / 'minimal'; tools.mkdir()
        for name in ('sh','mkdir','rmdir','rm','mktemp','awk','cut','od','tr','tar','gzip','grep','mv','shasum'):
            path = shutil.which(name); self.assertIsNotNone(path, name)
            (tools / name).symlink_to(path)
        (tools / 'curl').symlink_to(self.fake / 'curl')
        self.run_install(env=dict(self.env, PATH=str(tools)))
        self.assertTrue((self.parent / self.args[2] / 'bin/kicad-cli').is_file())

    def test_existing_runtime_preserved_before_network(self):
        self.fixture(); target = self.parent / self.args[2]; target.mkdir(parents=True)
        sentinel = target / 'user-file'; sentinel.write_bytes(b'keep')
        self.run_install(False)
        self.assertEqual(sentinel.read_bytes(), b'keep')
        self.assertFalse((self.root / 'requests').exists())

    def test_unsupported_refused_before_directory_or_network(self):
        self.fixture(); self.args[1] = 'linux-arm64'
        self.run_install(False)
        self.assertFalse(self.parent.exists())
        self.assertFalse((self.root / 'requests').exists())

    def test_wrong_pinned_hash_preserves_other_version(self):
        self.fixture(); self.args[5] = '0' * 64
        old = self.parent / '10.99.0-bp.3'; old.mkdir(parents=True)
        (old / 'preserved').write_bytes(b'previous')
        self.run_install(False)
        self.assertEqual((old / 'preserved').read_bytes(), b'previous')
        self.assertFalse((self.parent / self.args[2]).exists())

    def test_release_sums_must_agree(self):
        self.fixture(); Path(self.env['MOCK_SUMS']).write_text('0'*64 + '  ' + self.args[4] + '\n')
        self.run_install(False)
        self.assertFalse((self.parent / self.args[2]).exists())

    def test_wrong_header_even_with_matching_hash(self):
        archive = self.fixture(); archive.write_bytes(b'<html>upstream error</html>')
        self.args[5] = hashlib.sha256(archive.read_bytes()).hexdigest()
        Path(self.env['MOCK_SUMS']).write_text(self.args[5] + '  ' + self.args[4] + '\n')
        r = self.run_install(False); self.assertIn('header', r.stderr)

    def test_partial_download_leaves_no_install(self):
        self.fixture(); self.env['MOCK_DOWNLOAD_FAIL'] = '1'
        self.run_install(False)
        self.assertFalse((self.parent / self.args[2]).exists())

    def test_incomplete_layout_not_promoted(self):
        self.fixture(change=lambda files: files.pop('share/backplane/api.desc'))
        self.run_install(False)
        self.assertFalse((self.parent / self.args[2]).exists())

    def test_other_platform_manifest_not_promoted(self):
        self.fixture(change=lambda files: files.update({'manifest.json':b'{"version":"10.99.0-bp.4","platform":"darwin-arm64"}'}))
        self.run_install(False)
        self.assertFalse((self.parent / self.args[2]).exists())

    def test_path_traversal_rejected_before_unpack(self):
        self.fixture(member='../outside')
        r = self.run_install(False); self.assertIn('Unsafe', r.stderr)
        self.assertFalse((self.parent / 'outside').exists())

    def test_wrong_root_rejected_before_unpack(self):
        self.fixture(member='unexpected/file')
        r = self.run_install(False); self.assertIn('root', r.stderr)

    def test_other_installer_lock_preserved(self):
        self.fixture(); self.parent.mkdir(parents=True)
        lock = self.parent / ('.install-' + self.args[2] + '.lock'); lock.mkdir()
        r = subprocess.run(['sh','-c',SCRIPT,'sh']+self.args, env=self.env,
                           capture_output=True,text=True,timeout=10)
        self.assertNotEqual(r.returncode,0); self.assertTrue(lock.is_dir())
        self.assertFalse((self.root / 'requests').exists())

    def windows_tools(self, stub_powershell=False):
        # Exercise the exact ZIP branch on POSIX too. Only native path
        # conversion and PowerShell are stand-ins; curl is already offline.
        # There is deliberately no powershell.exe on this PATH.
        tools = self.root / 'closed-path'; tools.mkdir()
        for name in ('sh', 'mkdir', 'rmdir', 'rm', 'mktemp', 'awk', 'cut',
                     'od', 'tr', 'grep', 'mv', 'sha256sum'):
            tool = shutil.which(name); self.assertIsNotNone(tool, name)
            (tools / name).symlink_to(tool)
        (tools / 'curl').symlink_to(self.fake / 'curl')
        system = self.root / 'Windows system 日本'
        self.env['MOCK_SYSTEM'] = str(system)
        cygpath = tools / 'cygpath'
        if IS_CYGWIN:
            cygpath.symlink_to(shutil.which('cygpath'))
            return dict(self.env, PATH=str(tools))
        cygpath.write_text('#!' + sys.executable + '\n' + '''import os,sys
if sys.argv[1:] == ['-S']: print(os.environ['MOCK_SYSTEM'])
elif len(sys.argv) == 3 and sys.argv[1] in ('-w','-u'):
 p=sys.argv[2]
 if sys.argv[1]=='-u' and p==os.environ.get('MOCK_NATIVE_PARENT'): p=os.environ['MOCK_POSIX_PARENT']
 print(p)
else: sys.exit(89)
''')
        cygpath.chmod(0o700)
        if stub_powershell:
            ps = system / 'WindowsPowerShell/v1.0/powershell.exe'
            ps.parent.mkdir(parents=True)
            ps.write_text('#!' + sys.executable + '\n' + '''import json,os,sys,zipfile
from pathlib import Path
assert sys.argv[1:4] == ['-NoProfile','-NonInteractive','-Command'] and len(sys.argv) == 5
Path(os.environ['MOCK_PS_LOG']).write_text(json.dumps({
 'exe':sys.argv[0], 'root':os.environ['BP_KICAD_ROOT'],
 'zip':os.environ['BP_KICAD_ZIP'], 'out':os.environ['BP_KICAD_OUT']}))
with zipfile.ZipFile(os.environ['BP_KICAD_ZIP']) as z:
 z.extractall(os.environ['BP_KICAD_OUT'])
''')
            ps.chmod(0o700)
        return dict(self.env, PATH=str(tools), MOCK_PS_LOG=str(self.root / 'powershell-call'))

    @unittest.skipIf(IS_CYGWIN, 'POSIX stand-ins; real Cygwin coverage below')
    def test_windows_zip_resolves_system_powershell_off_path(self):
        self.fixture('windows-x86_64', 'zip')
        env = self.windows_tools(stub_powershell=True)
        self.run_install(env=env)
        call = json.loads((self.root / 'powershell-call').read_text())
        self.assertEqual(call['exe'], str(Path(env['MOCK_SYSTEM']) / 'WindowsPowerShell/v1.0/powershell.exe'))
        self.assertEqual(call['root'], self.args[4][:-4])
        self.assertTrue(call['zip'].endswith(self.args[4]))
        self.assertTrue(call['out'].endswith('/unpack'))
        self.assertEqual((self.parent / self.args[2] / 'bin/kicad-cli.exe').read_bytes(), b'MZfixture')

    @unittest.skipIf(IS_CYGWIN, 'POSIX stand-ins; real Cygwin coverage below')
    def test_windows_native_parent_with_backslashes_and_spaces(self):
        self.fixture('windows-x86_64', 'zip')
        env = self.windows_tools(stub_powershell=True)
        native = 'C:\\backplane_kicad\\new tools\\test\\'
        env.update(MOCK_NATIVE_PARENT=native, MOCK_POSIX_PARENT=str(self.parent))
        args = [native, *self.args[1:]]
        self.run_install(args=args, env=env)
        self.assertEqual((self.parent / self.args[2] / 'bin/kicad-cli.exe').read_bytes(), b'MZfixture')

    @unittest.skipIf(IS_CYGWIN, 'POSIX stand-ins; real Cygwin coverage below')
    def test_windows_zip_missing_system_powershell_cleans_stage(self):
        self.fixture('windows-x86_64', 'zip')
        env = self.windows_tools()
        # An ambient executable must never replace the system PowerShell.
        foreign = Path(env['PATH']) / 'powershell.exe'
        foreign.write_text('#!/bin/sh\necho unexpected-ambient-powershell >&2\nexit 91\n')
        foreign.chmod(0o700)
        r = self.run_install(False, env=env)
        self.assertIn('Windows PowerShell', r.stderr)
        self.assertNotIn('unexpected-ambient', r.stderr)
        self.assertFalse((self.parent / self.args[2]).exists())

    @unittest.skipUnless(IS_CYGWIN, 'actual PowerShell ZIP extraction runs on Cygwin')
    def test_windows_zip_actual_powershell(self):
        self.fixture('windows-x86_64', 'zip'); self.run_install()
        self.assertEqual((self.parent / self.args[2] / 'bin/kicad-cli.exe').read_bytes(), b'MZfixture')

    @unittest.skipUnless(IS_CYGWIN, 'actual closed-PATH PowerShell ZIP extraction runs on Cygwin')
    def test_windows_zip_actual_powershell_without_powershell_on_path(self):
        self.fixture('windows-x86_64', 'zip')
        self.run_install(env=self.windows_tools())
        self.assertEqual((self.parent / self.args[2] / 'bin/kicad-cli.exe').read_bytes(), b'MZfixture')

    @unittest.skipUnless(IS_CYGWIN, 'actual PowerShell ZIP extraction runs on Cygwin')
    def test_windows_zip_traversal_refused(self):
        self.fixture('windows-x86_64', 'zip', member='../outside')
        self.run_install(False)
        self.assertFalse((self.parent / self.args[2]).exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
