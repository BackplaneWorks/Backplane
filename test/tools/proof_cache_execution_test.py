"""Plan/join/contract controls with toy tool identities; NOT real proof passes.

These tests never substitute a checker in the execution path. Actual cold,
warm, changed and false-theorem execution is measured separately on Boat.
"""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


fixture = load('proof_fixture', Path(__file__).with_name('content_cache_test.py'))
pc = load('proof_cache_execution', ROOT/'scripts/proof-cache.py')
guard = load('proof_cache_guard', ROOT/'scripts/heavy-guard.py')


class Plans(unittest.TestCase):
    setUp = fixture.Shards.setUp
    write = fixture.Shards.write
    generate = fixture.Shards.generate

    def prepare(self):
        patcher = patch.object(pc, 'cc', fixture.cc)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.cache = self.root/'cache'
        return pc.make_plan(self.root, self.folder)

    def mark_metadata_fixture(self, count=None):
        # Synthetic compiler identity "A" cannot match a production cache key.
        units, _ = fixture.cc.shard_units(self.root, self.folder)
        for key, body in list(units.values())[:count]:
            fixture.cc._record(key, body, self.cache)

    def test_plan_contains_complete_verified_keyset(self):
        planned = self.prepare()
        self.assertEqual(set(planned['keys']), {'000', '001'})
        self.assertEqual(pc.validate(self.root, planned)[1].keys(), planned['keys'].keys())

    def test_cold_and_partial_cache_cannot_join(self):
        planned = self.prepare()
        self.assertEqual(pc.join(self.root, planned, self.cache)[1], 1)
        self.mark_metadata_fixture(1)
        result, status = pc.join(self.root, planned, self.cache)
        self.assertEqual((status, len(result['hit']), len(result['miss'])), (1, 1, 1))

    def test_complete_authenticated_metadata_fixture_joins(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        result, status = pc.join(self.root, planned, self.cache)
        self.assertEqual((status, result['law_count'], result['miss']), (0, 2, []))

    def test_dropped_or_forged_key_refuses_instead_of_skipping(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        for keys in ({'000': planned['keys']['000']}, {**planned['keys'], '000': '0'*64}):
            with self.subTest(keys=keys), self.assertRaises(fixture.cc.CacheError):
                pc.join(self.root, {**planned, 'keys': keys}, self.cache)

    def test_edited_source_and_stale_manifest_refuse(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        self.write('src/unused.bend', 'import Base\ndef unused() -> U32:\n  7\n')
        with self.assertRaises(ValueError): pc.join(self.root, planned, self.cache)
        self.generate()
        with self.assertRaises(fixture.cc.CacheError): pc.join(self.root, planned, self.cache)

    def test_changed_cached_source_during_lookup_is_caught(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        original = fixture.cc.plan
        def edited(*args, **kwargs):
            result = original(*args, **kwargs)
            self.write('src/unused.bend', 'import Base\ndef unused() -> U32:\n  9\n')
            return result
        with patch.object(fixture.cc, 'plan', side_effect=edited):
            with self.assertRaises(ValueError): pc.join(self.root, planned, self.cache)

    def test_deleted_or_tampered_mac_is_a_miss(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        entry = fixture.cc.entry_path(self.cache, next(iter(planned['keys'].values())))
        original = entry.read_text()
        entry.write_text(original.replace('passed', 'failed'))
        self.assertEqual(pc.join(self.root, planned, self.cache)[1], 1)
        entry.unlink()
        self.assertEqual(pc.join(self.root, planned, self.cache)[1], 1)

    def test_join_does_not_lookup_again_after_snapshot_fence(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        with patch.object(fixture.cc, 'plan', wraps=fixture.cc.plan) as lookup:
            self.assertEqual(pc.join(self.root, planned, self.cache)[1], 0)
            self.assertEqual(lookup.call_count, 1)

    def test_cached_worker_revalidates_source_after_lookup(self):
        planned = self.prepare()
        self.mark_metadata_fixture()
        admitted = types.SimpleNamespace(inherited_lock=lambda p: 7, lock_path=lambda: Path('/unused'))
        original = fixture.cc.lookup
        def edited(*args, **kwargs):
            hit = original(*args, **kwargs)
            self.write('src/unused.bend', 'import Base\ndef unused() -> U32:\n  9\n')
            return hit
        with patch.object(pc, 'module', side_effect=lambda name: admitted if name == 'heavy-guard.py' else fixture.cc._module(name)), \
             patch.object(pc, 'resource_state', return_value={}), \
             patch.object(fixture.cc, 'lookup', side_effect=edited), \
             patch.object(fixture.cc, '_record') as record:
            with self.assertRaises(ValueError):
                pc.run_one(self.root, planned, '000', self.root/'unused.json', self.cache)
            record.assert_not_called()

    def test_partition_cannot_escape_root(self):
        planned = self.prepare()
        with self.assertRaises(ValueError): pc.validate(self.root, {**planned, 'partition': '../elsewhere'})

    def test_unadmitted_executor_never_reads_declared_pass_or_records(self):
        planned = self.prepare()
        fake = self.root/'claimed.json'
        fake.write_text(json.dumps({'state': 'passed', 'output': 'All terms check.'}))
        denied = types.SimpleNamespace(inherited_lock=lambda p: None, lock_path=lambda: Path('/unused'))
        with patch.object(pc, 'module', return_value=denied), patch.object(fixture.cc, '_record') as record:
            with self.assertRaises(fixture.cc.CacheError):
                pc.run_one(self.root, planned, '000', fake, self.cache)
            record.assert_not_called()

    def test_legacy_receipt_import_is_refused_without_recording(self):
        self.prepare()
        with patch.object(fixture.cc, '_record') as record:
            with self.assertRaisesRegex(fixture.cc.CacheError, 'caller-declared'):
                fixture.cc.main(['record-proof', str(self.folder), '--receipt', 'claimed.json'])
            record.assert_not_called()


class WorkerContracts(unittest.TestCase):
    def command(self):
        return [sys.executable, str(ROOT/'scripts/proof-cache.py'), 'run',
                '--plan', 'plan.json', '--shard', '000', '--result', 'receipts/000.json']

    def test_only_exact_single_shard_route_gets_bounded_profile(self):
        good = self.command()
        self.assertIsNotNone(guard.worker_contract('proof-shard', good))
        for command in (good+['--checker', 'anything'], good+['--shard', '001'],
                        good+['--all', '1'], good[:-2], [*good[:2], 'join', *good[3:]],
                        [good[0], '/elsewhere/proof-cache.py', *good[2:]]):
            with self.subTest(command=command):
                self.assertIsNone(guard.worker_contract('proof-shard', command))
        self.assertIsNone(guard.worker_contract('compile-shard', good))

    def test_uncapped_or_held_only_route_cannot_start_worker(self):
        with patch.object(guard, 'verify_shard_cgroup', return_value=False), \
             patch.object(guard.os, 'execvpe') as execute:
            self.assertEqual(guard.main(['proof-shard', '--', *self.command()]), 75)
            execute.assert_not_called()
        self.assertEqual(guard.main(['--held', 'proof-shard', '--', *self.command()]), 2)

    def test_receipt_shape_requires_identity_resources_and_real_verdict(self):
        ps = fixture.cc._module('proof-shards.py')
        maximum = 3699998720
        valid = {'shard': '000', 'manifest_sha256': 'm', 'state': 'passed', 'inputs_unchanged': True,
                 'actual_bend_flock_held': True, 'memory_max_bytes': maximum, 'seconds': 1,
                 'peak_rss_kib': 123, 'aggregate_peak_bytes': 200000,
                 'swap_before': {'pswpin': 0, 'pswpout': 0},
                 'swap_after': {'pswpin': 0, 'pswpout': 0}, 'exit': 0, 'output': 'All terms check.\n'}
        self.assertTrue(pc.receipt_valid(valid, '000', 'm', ps, maximum))
        for change in ({'state': 'failed'}, {'output': 'All terms check.\nError: bad\n'},
                       {'memory_max_bytes': 8*1024**3}, {'aggregate_peak_bytes': maximum+1},
                       {'peak_rss_kib': True}, {'seconds': float('nan')},
                       {'inputs_unchanged': False}, {'actual_bend_flock_held': False},
                       {'swap_after': {'pswpin': 0, 'pswpout': 1}}, {'shard': '001'}):
            with self.subTest(change=change):
                self.assertFalse(pc.receipt_valid({**valid, **change}, '000', 'm', ps, maximum))


if __name__ == '__main__':
    unittest.main()
