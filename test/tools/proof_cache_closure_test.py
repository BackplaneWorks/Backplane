"""Cache mutations against real canonical planning; no Bend execution needed."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'cache_fixture', Path(__file__).with_name('content_cache_test.py'))
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
cc = fixture.cc


class ProofClosure(unittest.TestCase):
    setUp = fixture.Shards.setUp
    write = fixture.Shards.write
    generate = fixture.Shards.generate

    def units(self):
        return cc.shard_units(self.root, self.folder)[0]

    def keys(self):
        units, manifest = cc.shard_units(self.root, self.folder)
        return {tuple(s['primary_laws']): units[s['id']][0]
                for s in manifest['shards']}

    def edit(self, file, old, new):
        text = (self.root/file).read_text()
        self.assertIn(old, text)
        self.write(file, text.replace(old, new))

    def test_one_proof_edit_retains_unrelated_shard(self):
        before = self.keys()
        self.edit('PROOF.bend', 'def Laws.second():\n  {==}',
                  'def Laws.second():\n  {==}\n')
        self.generate()
        after = self.keys()
        self.assertEqual(before[('first',)], after[('first',)])
        self.assertNotEqual(before[('second',)], after[('second',)])

    def test_false_statement_invalidates_only_affected_shard(self):
        before = self.keys()
        self.edit('LAWS.bend', '{1n == 1n : Nat}', '{1n == 2n : Nat}')
        with self.assertRaises(ValueError):
            self.keys()  # stale partition fails before lookup
        self.generate()
        after = self.keys()
        self.assertEqual(before[('first',)], after[('first',)])
        self.assertNotEqual(before[('second',)], after[('second',)])

    def test_old_authenticated_pass_cannot_hide_changed_theorem(self):
        with tempfile.TemporaryDirectory() as cache:
            before = self.units()
            for key, body in before.values():
                cc._record(key, body, cache)  # test-only trusted pass fixture
            self.assertEqual(len(cc.plan(before, True, cache)[0]), 2)
            self.edit('LAWS.bend', '{1n == 1n : Nat}', '{1n == 2n : Nat}')
            self.generate()
            hit, miss = cc.plan(self.units(), True, cache)
            self.assertEqual((len(hit), len(miss)), (1, 1))

    def test_local_proof_dependency_invalidates_dependents(self):
        self.edit('PROOF.bend', 'def Laws.first():\n  {==}',
                  'def Laws.first():\n  Laws.second()')
        self.generate()
        before = self.keys()
        self.edit('LAWS.bend', '{1n == 1n : Nat}', '{1n == 2n : Nat}')
        self.generate()
        after = self.keys()
        self.assertTrue(all(before[n] != after[n] for n in before))

    def test_unused_import_is_still_checked_and_keyed(self):
        before = self.keys()
        self.edit('src/unused.bend', '  0\n', '  7\n')
        self.generate()
        after = self.keys()
        self.assertEqual(sum(before[n] != after[n] for n in before), 1)

    def test_shared_import_change_invalidates_both_readers(self):
        self.edit('LAWS.bend', '{0n == 0n : Nat}', '{U.unused() == 0 : U32}')
        self.edit('LAWS.bend', '{1n == 1n : Nat}', '{U.unused() == 0 : U32}')
        self.generate()
        before = self.keys()
        self.edit('src/unused.bend', '  0\n', '  7\n')
        self.generate()
        after = self.keys()
        self.assertTrue(all(before[n] != after[n] for n in before))

    def test_transitive_import_of_canonical_proof_is_not_omitted(self):
        self.edit('src/unused.bend', 'import Base\n',
                  'import Base\nimport ../PROOF.bend as Canon\n')
        self.generate()
        before = self.keys()
        self.edit('PROOF.bend', 'def Laws.second():\n  {==}',
                  'def Laws.second():\n  {==}\n')
        self.generate()
        after = self.keys()
        # The first shard imports unused.bend to preserve the unused import;
        # that module reads the ENTIRE canonical PROOF, including second.
        self.assertTrue(all(before[n] != after[n] for n in before))
        bodies = [body for _, body in self.units().values()]
        self.assertTrue(any('src/PROOF.bend' in b['inputs'] for b in bodies))

    def test_transitive_import_edit_invalidates_reader(self):
        self.write('src/leaf.bend', 'import Base\ndef leaf() -> U32:\n  0\n')
        self.edit('src/unused.bend', 'import Base\n',
                  'import Base\nimport ./leaf.bend as Leaf\n')
        self.generate()
        before = self.keys()
        self.edit('src/leaf.bend', '  0\n', '  1\n')
        self.generate()
        after = self.keys()
        self.assertEqual(sum(before[n] != after[n] for n in before), 1)

    def test_source_mode_change_invalidates_reader(self):
        before = self.keys()
        path = self.root/'src/unused.bend'
        path.chmod(path.stat().st_mode ^ 0o100)
        with self.assertRaises(ValueError):
            self.keys()
        self.generate()
        after = self.keys()
        self.assertEqual(sum(before[n] != after[n] for n in before), 1)

    def test_generated_mode_change_invalidates_reader(self):
        before = self.keys()
        path = self.folder/'000/PROOF.bend'
        path.chmod(path.stat().st_mode ^ 0o100)
        self.assertNotEqual(before, self.keys())

    def test_canonical_mode_change_invalidates_keys(self):
        before = self.keys()
        path = self.root/'PROOF.bend'
        path.chmod(path.stat().st_mode ^ 0o100)
        self.generate()
        after = self.keys()
        self.assertTrue(all(before[n] != after[n] for n in before))

    def test_logic_identity_changes_invalidate_all(self):
        original = self.ps.plan
        for name in ('planner_sha256', 'verdict_sha256', 'guard_sha256'):
            with self.subTest(name=name):
                self.generate()
                before = self.keys()
                def changed(*args):
                    manifest, files = original(*args)
                    manifest[name] = 'changed-policy'
                    return manifest, files
                with patch.object(self.ps, 'plan', side_effect=changed):
                    self.generate()
                    after = self.keys()
                    self.assertTrue(all(before[n] != after[n] for n in before))

    def test_renumbered_identical_slots_keep_keys(self):
        # Simulate a verified planner renumbering, while retaining each shard's
        # exact declarations/imports/bytes. Numeric slot is scheduling metadata.
        before = self.keys()
        original = self.ps.plan
        def swapped(*args):
            manifest, files = original(*args)
            translated = {}
            for shard in manifest['shards']:
                old = shard['id']
                new = '001' if old == '000' else '000'
                shard['id'] = new
                shard['files'] = {new+'/'+n.split('/', 1)[1]: d
                                  for n, d in shard['files'].items()}
            for name, body in files.items():
                old, rest = name.split('/', 1)
                translated[('001' if old == '000' else '000')+'/'+rest] = body
            return manifest, translated
        with patch.object(self.ps, 'plan', side_effect=swapped):
            self.generate()
            self.assertEqual(before, self.keys())

    def test_forged_dropped_law_manifest_fails_before_lookup(self):
        path = self.folder/'manifest.json'
        manifest = json.loads(path.read_text())
        manifest['shards'][0]['primary_laws'] = []
        path.write_text(json.dumps(manifest))
        with self.assertRaises(ValueError):
            self.units()

    def test_corrupt_or_missing_generated_proof_fails(self):
        path = self.folder/'000/PROOF.bend'
        path.write_text('import Base\n')
        with self.assertRaises(ValueError):
            self.units()
        path.unlink()
        with self.assertRaises(ValueError):
            self.units()

    def test_unknown_dependency_syntax_fails(self):
        self.edit('src/unused.bend', 'import Base\n',
                  'import Base\nimport <opaque> as X\n')
        with self.assertRaises((ValueError, cc.CacheError, fixture.subprocess.CalledProcessError)):
            self.generate()


if __name__ == '__main__':
    unittest.main()
