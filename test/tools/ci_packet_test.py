import contextlib
import fnmatch
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('packet', ROOT/'scripts/ci-packet.py')
packet = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packet)


class PacketTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        base = Path(self.temp.name)
        self.folder = base/'repo'
        self.folder.mkdir()
        self.old = Path.cwd()
        os.chdir(self.folder)
        self.addCleanup(os.chdir, self.old)
        self.addCleanup(self.temp.cleanup)
        for f in [packet.SELF, packet.CLOSURE]:
            Path(f).parent.mkdir(parents=True, exist_ok=True)
            Path(f).write_bytes((ROOT/f).read_bytes())
        Path(packet.PIN).parent.mkdir(parents=True)
        Path(packet.PIN).write_text('pinned toolchain\n')
        Path('.gitignore').write_text('build/\n')
        Path('root.bend').write_text('import ./lib.bend as L\n')
        Path('lib.bend').write_text('import Base\n')
        self.binary = base/'tool/bin/bend'
        self.binary.parent.mkdir(parents=True)
        self.binary.write_text('#!/bin/sh\nexit 1\n')
        self.binary.chmod(0o755)
        self.library = base/'tool/bend2/base.bend'
        self.library.parent.mkdir()
        self.library.write_text('compiler resource\n')
        self.env = patch.dict(os.environ, {'PATH': str(self.binary.parent)+os.pathsep+os.environ['PATH'],
                                          'BEND_LIB': str(base/'named')})
        self.env.start()
        self.addCleanup(self.env.stop)
        subprocess.run(['git', 'init', '-q'], check=True)
        subprocess.run(['git', 'add', '.'], check=True)
        subprocess.run(['git', '-c', 'user.name=fixture', '-c', 'user.email=fixture@localhost',
                        'commit', '-qm', 'fixture'], check=True)
        with contextlib.redirect_stdout(io.StringIO()):
            packet.create(['root.bend'], [], 'packet.json', [])
        self.key = packet.digest('packet.json')

    def test_closure_and_exact_packet(self):
        data = packet.verify('packet.json', self.key)
        self.assertIn('lib.bend', data['files'])
        Path('unexpected.bend').write_text('import Base\n')
        with self.assertRaisesRegex(ValueError, 'unexpected source'):
            packet.verify('packet.json', self.key)

    def test_dependency_mutation_invalidates(self):
        Path('lib.bend').write_text('different\n')
        with self.assertRaisesRegex(ValueError, 'input changed: lib.bend'):
            packet.verify('packet.json', self.key)

    def test_compiler_and_resource_mutations_invalidate(self):
        before = self.binary.read_bytes()
        self.binary.write_bytes(before+b'# changed\n')
        with self.assertRaisesRegex(ValueError, 'compiler/library identity'):
            packet.verify('packet.json', self.key)
        self.binary.write_bytes(before)
        self.library.write_text('changed library\n')
        with self.assertRaisesRegex(ValueError, 'compiler/library identity'):
            packet.verify('packet.json', self.key)

    def test_named_library_addition_invalidates(self):
        folder = Path(os.environ['BEND_LIB'])
        folder.mkdir()
        (folder/'new.bend').write_text('new library')
        with self.assertRaisesRegex(ValueError, 'compiler/library identity'):
            packet.verify('packet.json', self.key)

    def test_manifest_change_is_not_accepted(self):
        data = json.loads(Path('packet.json').read_text())
        del data['files']['lib.bend']
        Path('packet.json').write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, 'manifest hash mismatch'):
            packet.verify('packet.json', self.key)

    def test_failed_command_cannot_turn_green(self):
        with contextlib.redirect_stdout(io.StringIO()):
            rc = packet.execute('packet.json', self.key, [sys.executable, '-c', 'raise SystemExit(7)'])
        self.assertEqual(rc, 7)
        result = json.loads(Path('build/ci-packet/result.json').read_text())
        self.assertEqual(result['returncode'], 7)

    def test_command_mutation_leaves_no_stale_pass(self):
        out = Path('build/ci-packet/result.json')
        out.parent.mkdir(parents=True)
        out.write_text('{"returncode":0}')
        with self.assertRaisesRegex(ValueError, 'input changed'):
            packet.execute('packet.json', self.key,
                           [sys.executable, '-c', "open('lib.bend','w').write('bad')"])
        self.assertFalse(out.exists())

    def test_symlinks_and_traversal_refused(self):
        Path('linked.bend').symlink_to('lib.bend')
        with self.assertRaisesRegex(ValueError, 'symbolic links'):
            packet.source('linked.bend')
        for name in ['../file', '/tmp/file', 'a/../file', './file', 'a//file', 'a\nfile']:
            with self.assertRaises(ValueError):
                packet.safe_path(name)

    def test_explicit_generated_root_can_be_ignored(self):
        Path('build').mkdir()
        Path('build/generated.bend').write_text('import ../lib.bend as L\n')
        with contextlib.redirect_stdout(io.StringIO()):
            packet.create(['build/generated.bend'], [], 'generated.json', [])
        data = json.loads(Path('generated.json').read_text())
        self.assertIn('build/generated.bend', data['files'])
        self.assertIn('lib.bend', data['files'])

    def test_generated_manifest_under_build_is_still_hash_verified(self):
        Path('packet.json').unlink()
        name = 'build/packets/packet.json'
        with contextlib.redirect_stdout(io.StringIO()):
            packet.create(['root.bend'], [], name, [])
        key = packet.digest(name)
        packet.verify(name, key)
        Path('unexpected.bend').write_text('import Base\n')
        with self.assertRaisesRegex(ValueError, 'unexpected source'):
            packet.verify(name, key)
        Path('unexpected.bend').unlink()
        Path(name).write_text('{}')
        with self.assertRaisesRegex(ValueError, 'manifest hash mismatch'):
            packet.verify(name, key)

    def test_archive_cannot_bypass_private_file_filter(self):
        Path('.env').write_text('private fixture, never packed')
        with self.assertRaisesRegex(ValueError, 'private/generated capture'):
            packet.create(['root.bend'], ['.env'], 'private.json', [])
        self.assertFalse(Path('private.json').exists())

    def test_gitignore_cannot_hide_unexpected_packet_source(self):
        Path('build/unused').mkdir(parents=True)
        Path('hidden').mkdir()
        Path('hidden/.gitignore').write_text('*\n')
        Path('hidden/extra.bend').write_text('import Base\n')
        with self.assertRaisesRegex(ValueError, 'unexpected source'):
            packet.verify('packet.json', self.key)

    def test_exclusions_are_literal(self):
        for name in ['test/a[1].bend', 'a?b', 'a*b']:
            pattern = packet.literal_pattern(name)
            self.assertTrue(fnmatch.fnmatch(name, pattern))
            self.assertFalse(fnmatch.fnmatch(name+'x', pattern))

    def test_transport_cache_is_deterministic_and_repairs_corruption(self):
        with contextlib.redirect_stdout(io.StringIO()):
            archive = packet.bundle('packet.json', self.key)
            before = packet.digest(archive)
            packet.bundle('packet.json', self.key)
            self.assertEqual(packet.digest(archive), before)
            archive.write_bytes(b'corrupt transport cache')
            packet.bundle('packet.json', self.key)
        self.assertEqual(packet.digest(archive), before)

    def test_exact_permissions_survive_transport_and_mode_changes_invalidate(self):
        Path('lib.bend').chmod(0o600)
        with contextlib.redirect_stdout(io.StringIO()):
            packet.create(['root.bend'], [], 'packet.json', [])
            key = packet.digest('packet.json')
            archive = packet.bundle('packet.json', key)
        Path('lib.bend').chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'input changed'):
            packet.verify('packet.json', key, complete=False)
        packet.unpack(str(archive), packet.digest(archive), 'packet.json', key)
        self.assertEqual(Path('lib.bend').stat().st_mode & 0o777, 0o600)

    def test_unpack_verifies_and_restores_exact_bytes(self):
        with contextlib.redirect_stdout(io.StringIO()):
            archive = packet.bundle('packet.json', self.key)
        checksum = packet.digest(archive)
        Path('lib.bend').write_text('changed after packing')
        packet.unpack(str(archive), checksum, 'packet.json', self.key)
        self.assertEqual(Path('lib.bend').read_text(), 'import Base\n')
        packet.verify('packet.json', self.key, transport={str(archive): checksum})
        archive.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'transport archive hash mismatch'):
            packet.unpack(str(archive), checksum, 'packet.json', self.key)

    def test_traversal_and_duplicate_archive_entries_are_rejected(self):
        for names in [['../escaped'], ['.git/config'], ['lib.bend', 'lib.bend']]:
            with tarfile.open('bad.tar.gz', 'w:gz') as t:
                for name in names:
                    m = tarfile.TarInfo(name)
                    m.size = 1
                    t.addfile(m, io.BytesIO(b'x'))
            with self.assertRaises(ValueError):
                packet.archive_members('bad.tar.gz', 'packet.json', self.key)


if __name__ == '__main__':
    unittest.main()
