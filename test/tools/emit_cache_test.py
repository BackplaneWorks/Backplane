"""Hermetic raw-emission cache tests: no real Bend/Bun, home, install or IO service."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


cache = load('emit_cache', ROOT/'scripts/emit-cache.py')
local = load('emit_cache_local', ROOT/'scripts/local.py')
KEY = 'a'*40
RAW = b'console.log("ok");\n'
CHANGED = b'console.log("NO");\n'


class CacheIntegrity(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.store = self.root/'cache'
        self.source = self.root/'emit.js'
        self.source.write_bytes(RAW)
        cache.publish(self.store, KEY, self.source)
        self.data, self.receipt = cache.paths(self.store, KEY)
        self.output = self.root/'work/all.js'

    def assert_miss(self):
        self.assertFalse(cache.verify(self.store, KEY))
        self.assertFalse(cache.restore(self.store, KEY, self.output))
        self.assertFalse(self.output.exists())
        self.assertEqual(list(self.root.rglob('.emit-*')), [])

    def test_valid_cache_publishes_and_restores_exact_bytes(self):
        self.assertTrue(cache.verify(self.store, KEY))
        self.assertTrue(cache.restore(self.store, KEY, self.output))
        self.assertEqual(self.output.read_bytes(), RAW)
        self.assertEqual(json.loads(self.receipt.read_text()),
            {'schema': cache.SCHEMA, 'key': KEY, 'bytes': len(RAW), 'sha256': hashlib.sha256(RAW).hexdigest()})

    def test_same_size_corruption_is_a_miss(self):
        self.assertEqual(len(RAW), len(CHANGED))
        self.data.write_bytes(CHANGED)
        self.assert_miss()

    def test_legacy_partial_missing_and_oversized_receipts_are_misses(self):
        for value in (str(len(RAW)), '{', 'null', '['*1100+']'*1100, 'x'*4097):
            with self.subTest(value=value[:20]):
                self.receipt.write_text(value)
                self.assert_miss()
        self.receipt.unlink()
        self.assert_miss()

    def test_receipt_fields_are_strict_and_bound_to_source_key(self):
        original = json.loads(self.receipt.read_text())
        for overrides in ({'key': 'b'*40}, {'schema': 'old'}, {'bytes': True},
                          {'bytes': len(RAW)+1}, {'sha256': '0'*64}, {'sha256': 'bad'}, {'extra': 1}):
            with self.subTest(overrides=overrides):
                self.receipt.write_text(json.dumps({**original, **overrides}))
                self.assert_miss()
        self.receipt.write_text(json.dumps(original)[:-1]+',"key":"'+KEY+'"}')
        self.assert_miss()

    def test_truncated_missing_empty_or_linked_data_are_misses(self):
        for value in (RAW[:-1], b''):
            self.data.write_bytes(value)
            self.assert_miss()
        self.data.unlink()
        self.assert_miss()
        self.data.symlink_to(self.source)
        self.assert_miss()

    def test_nonregular_and_linked_receipts_refuse_without_waiting(self):
        self.receipt.unlink()
        self.receipt.symlink_to(self.source)
        self.assert_miss()
        self.receipt.unlink()
        os.mkfifo(self.receipt)
        self.assert_miss()

    def test_restore_uses_a_verified_snapshot(self):
        self.assertTrue(cache.restore(self.store, KEY, self.output))
        self.data.write_bytes(CHANGED)
        self.assertEqual(self.output.read_bytes(), RAW)
        self.assertFalse(cache.restore(self.store, KEY, self.output))
        self.assertEqual(self.output.read_bytes(), RAW)

    def test_changed_bytes_during_copy_never_publish_output(self):
        target_inode = self.data.stat().st_ino
        real_read = os.read
        changed = False
        def mutate(fd, size):
            nonlocal changed
            if os.fstat(fd).st_ino == target_inode and not changed:
                changed = True
                self.data.write_bytes(CHANGED)
                info = self.data.stat()
                os.utime(self.data, ns=(info.st_atime_ns, info.st_mtime_ns+1))
            return real_read(fd, size)
        with patch.object(cache.os, 'read', side_effect=mutate):
            self.assertFalse(cache.restore(self.store, KEY, self.output))
        self.assertTrue(changed)
        self.assertFalse(self.output.exists())
        self.assertEqual(list(self.root.rglob('.emit-*')), [])

    def test_concurrent_lru_touch_does_not_force_reemission(self):
        target_inode = self.data.stat().st_ino
        real_read = os.read
        touched = False
        def touch_after_read(fd, size):
            nonlocal touched
            block = real_read(fd, size)
            if block and os.fstat(fd).st_ino == target_inode and not touched:
                touched = True
                info = self.data.stat()
                os.utime(self.data, ns=(info.st_atime_ns, info.st_mtime_ns+1))
            return block
        with patch.object(cache.os, 'read', side_effect=touch_after_read):
            self.assertTrue(cache.restore(self.store, KEY, self.output))
        self.assertTrue(touched)
        self.assertEqual(self.output.read_bytes(), RAW)

    def test_partial_publish_cannot_authenticate_new_data_with_old_receipt(self):
        self.source.write_bytes(CHANGED)
        replace = os.replace
        def fail_receipt(source, target):
            if Path(target) == self.receipt:
                raise OSError('fixture receipt failure')
            return replace(source, target)
        with patch.object(cache.os, 'replace', side_effect=fail_receipt):
            with self.assertRaises(OSError): cache.publish(self.store, KEY, self.source)
        self.assert_miss()

    def test_expected_transfer_digest_binds_copied_bytes(self):
        self.source.write_bytes(CHANGED)
        with self.assertRaisesRegex(ValueError, 'transferred emission digest mismatch'):
            cache.publish(self.store, KEY, self.source, expected_sha256=hashlib.sha256(RAW).hexdigest())
        self.assertTrue(cache.verify(self.store, KEY))
        self.assertEqual(self.data.read_bytes(), RAW)
        self.assertEqual(list(self.root.rglob('.emit-*')), [])

    def test_invalid_key_cannot_escape_cache_directory(self):
        for key in ('../escape', 'a'*39, 'A'*40):
            self.assertFalse(cache.verify(self.store, key))
            self.assertFalse(cache.restore(self.store, key, self.output))
            with self.assertRaises(ValueError): cache.publish(self.store, key, self.source)

    def test_local_warm_hint_uses_the_same_integrity_and_legacy_miss_rules(self):
        env = {'BACKPLANE_EMIT_CACHE': str(self.store)}
        with patch.object(local.subprocess, 'check_output', return_value=KEY), \
             patch.object(local, 'phone_bridge_cached', return_value=True):
            self.assertTrue(local.warm_build(env))
            self.data.write_bytes(CHANGED)
            self.assertFalse(local.warm_build(env))
            self.data.write_bytes(RAW)
            self.receipt.write_text(str(len(RAW)))
            self.assertFalse(local.warm_build(env))
        with patch.object(local.subprocess, 'check_output') as key:
            self.assertFalse(local.warm_build({'BACKPLANE_EMIT_CACHE': '0'}))
            key.assert_not_called()

    def test_local_transfer_mutation_after_initial_check_is_not_published(self):
        expected = hashlib.sha256(RAW).hexdigest()
        original_digest = local.digest
        def changed_after_hash(path):
            value = original_digest(path)
            Path(path).write_bytes(CHANGED)
            return value
        with patch.object(local.subprocess, 'check_output', return_value=KEY), \
             patch.object(local, 'digest', side_effect=changed_after_hash):
            with self.assertRaisesRegex(ValueError, 'transferred emission digest mismatch'):
                local.import_emit(self.root, {'emit_key': KEY, 'emit_sha256': expected},
                                  {'BACKPLANE_EMIT_CACHE': str(self.store)})
        self.assertTrue(cache.verify(self.store, KEY))
        self.assertEqual(self.data.read_bytes(), RAW)


class BuildCacheHarness:
    """Exercise the real shell cache branch, stop at fake Bun/Bend before compile."""
    def __init__(self, root, build_source=None):
        self.root = Path(root)
        self.store = self.root/'cache'
        (self.root/'scripts').mkdir()
        (self.root/'bin').mkdir()
        (self.root/'scripts/build-js.sh').write_bytes(build_source or (ROOT/'scripts/build-js.sh').read_bytes())
        shutil.copyfile(ROOT/'scripts/emit-cache.py', self.root/'scripts/emit-cache.py')
        (self.root/'scripts/heavy-guard.sh').write_text('heavy_guard() { return 0; }\n')
        (self.root/'scripts/sidebar-draft-guard.py').write_text('')
        (self.root/'scripts/local-phase.py').write_text(
            'import os,sys\na=sys.argv[sys.argv.index("--")+1:]\nos.execvp(a[0],a)\n')
        key = self.root/'scripts/inputs-key.sh'
        key.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > key-argv\nprintf "%s\\n" '+KEY+'\n')
        key.chmod(0o755)
        self.env = {'PATH': str(self.root/'bin')+':'+os.environ['PATH'], 'HOME': str(self.root),
                    'BACKPLANE_EMIT_CACHE': str(self.store), 'PYTHONDONTWRITEBYTECODE': '1',
                    'BACKPLANE_TELEMETRY_BAKE': '0', 'BACKPLANE_GOOGLE_BAKE': '0'}
        self.runtimes()

    def runtimes(self, emit=False):
        for name, status in (('bun', 97), ('bend', 98)):
            file = self.root/'bin'/name
            content = '#!/bin/sh\nprintf reached > '+name+'.marker\n'
            if name == 'bend' and emit:
                content += 'while [ "$1" != -o ]; do shift; done\nprintf "fresh emission\\n" > "$2"\nexit 0\n'
            else:
                content += 'exit '+str(status)+'\n'
            file.write_text(content)
            file.chmod(0o755)

    def run(self, *args):
        return subprocess.run(['sh', 'scripts/build-js.sh', *(args or ('test/fixture.bend', 'dist/fixture'))],
            cwd=self.root, env=self.env, text=True, capture_output=True, timeout=5)


class BuildCacheRouting(unittest.TestCase):
    def test_valid_cache_reuses_without_emission(self):
        with tempfile.TemporaryDirectory() as directory:
            f = BuildCacheHarness(directory)
            source = f.root/'raw.js'; source.write_bytes(RAW)
            cache.publish(f.store, KEY, source)
            result = f.run()
            self.assertEqual(result.returncode, 97, result.stderr)
            self.assertFalse((f.root/'bend.marker').exists())
            self.assertEqual((f.root/'build/js/fixture/all.js').read_bytes(), RAW)

    def test_legacy_and_corrupt_cache_require_fresh_emission(self):
        for legacy in (True, False):
            with self.subTest(legacy=legacy), tempfile.TemporaryDirectory() as directory:
                f = BuildCacheHarness(directory)
                source = f.root/'raw.js'; source.write_bytes(RAW)
                cache.publish(f.store, KEY, source)
                if legacy:
                    cache.paths(f.store, KEY)[1].write_text(str(len(RAW)))
                else:
                    cache.paths(f.store, KEY)[0].write_bytes(CHANGED)
                result = f.run()
                self.assertEqual(result.returncode, 98, result.stderr)
                self.assertFalse((f.root/'bun.marker').exists())
                self.assertFalse((f.root/'build/js/fixture/all.js').exists())

    def test_fresh_fixture_emission_publishes_a_verifiable_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            f = BuildCacheHarness(directory); f.runtimes(emit=True)
            result = f.run()
            self.assertEqual(result.returncode, 97, result.stderr)
            self.assertTrue(cache.verify(f.store, KEY))
            self.assertEqual(cache.paths(f.store, KEY)[0].read_bytes(), b'fresh emission\n')

    def test_emit_source_key_domain_and_arguments_stay_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            f = BuildCacheHarness(directory)
            result = f.run('--key', 'test/fixture.bend')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((f.root/'key-argv').read_text().splitlines(),
                ['emit-js1:test/fixture.bend', 'scripts/inputs-key.sh', 'scripts/bend-closure.py'])
            self.assertEqual(result.stdout.strip(), KEY)


if __name__ == '__main__':
    unittest.main()
