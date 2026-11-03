import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('content_cache', ROOT/'scripts/content-cache.py')
cc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cc)

TOOL = {'toolchain': {'binary_sha256': 'bend-A', 'bundled': {'base.bend': '1'}, 'named_libraries': {}},
        'env': {'BEND_LIB': '', 'BENDTT': '', 'CC': ''}, 'logic': {'scripts/bendok.sh': 'v1'}}


class Base(unittest.TestCase):
    def setUp(self):
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        self.root = Path(t.name)/'repo'
        self.cache = Path(t.name)/'cache'
        self.shared = Path(t.name)/'shared'
        (self.root/'src').mkdir(parents=True)
        self.write('src/leaf.bend', 'import Base\ndef leaf() -> U32:\n  1\n')
        self.write('src/mid.bend', 'import Base\nimport ./leaf.bend as L\ndef mid() -> U32:\n  L.leaf()\n')
        self.write('src/top.bend', 'import Base\nimport ./mid.bend as M\nimport ./fx.js\ndef top() -> U32:\n  M.mid()\n')
        self.write('src/fx.js', 'export const x = 1\n')
        self.write('src/other.bend', 'import Base\ndef other() -> U32:\n  2\n')
        subprocess.run(['git', 'init', '-q'], cwd=self.root, check=True)
        env = patch.dict(os.environ, {'BP_CONTENT_CACHE': str(self.cache), 'BP_CONTENT_CACHE_SHARED': ''})
        env.start()
        self.addCleanup(env.stop)
        self.tool = dict(TOOL)
        p = patch.object(cc, 'tool_identity', side_effect=lambda: self.tool)
        p.start()
        self.addCleanup(p.stop)

    def write(self, name, text):
        (self.root/name).write_text(text)

    def keys(self, files=None):
        return {n: k for n, (k, _) in cc.check_units(self.root, files).items()}

    def mark_all_passed(self):
        for k, body in cc.check_units(self.root).values():
            cc.record(k, body)


class Keys(Base):
    def test_unchanged_hits(self):
        self.mark_all_passed()
        hit, miss = cc.plan(cc.check_units(self.root))
        self.assertEqual((len(hit), miss), (4, []))

    def test_dependency_change_invalidates_all_dependents_only(self):
        before = self.keys()
        self.write('src/leaf.bend', 'import Base\ndef leaf() -> U32:\n  3\n')
        after = self.keys()
        changed = {n for n in before if before[n] != after[n]}
        self.assertEqual(changed, {'src/leaf.bend', 'src/mid.bend', 'src/top.bend'})

    def test_stale_pass_is_not_returned_after_dependency_change(self):
        self.mark_all_passed()
        self.write('src/leaf.bend', 'import Base\ndef leaf() -> U32:\n  "wrong"\n')
        hit, miss = cc.plan(cc.check_units(self.root))
        self.assertEqual(sorted(miss), ['src/leaf.bend', 'src/mid.bend', 'src/top.bend'])
        self.assertEqual(hit, ['src/other.bend'])

    def test_effect_file_change_invalidates(self):
        before = self.keys()['src/top.bend']
        self.write('src/fx.js', 'export const x = 2\n')
        self.assertNotEqual(before, self.keys()['src/top.bend'])

    def test_c_include_change_invalidates(self):
        self.write('src/e.c', '#include "e.h"\n')
        self.write('src/e.h', 'int a;\n')
        self.write('src/c.bend', 'import Base\nimport ./e.c\ndef c() -> U32:\n  0\n')
        before = self.keys(['src/c.bend'])
        self.write('src/e.h', 'int b;\n')
        self.assertNotEqual(before, self.keys(['src/c.bend']))

    def test_compiler_identity_change_misses(self):
        self.mark_all_passed()
        self.tool = {**TOOL, 'toolchain': {**TOOL['toolchain'], 'binary_sha256': 'bend-B'}}
        hit, miss = cc.plan(cc.check_units(self.root))
        self.assertEqual(hit, [])

    def test_environment_and_verdict_logic_change_misses(self):
        self.mark_all_passed()
        self.tool = {**TOOL, 'env': {**TOOL['env'], 'BENDTT': '1'}}
        self.assertEqual(cc.plan(cc.check_units(self.root))[0], [])
        self.tool = {**TOOL, 'logic': {'scripts/bendok.sh': 'v2'}}
        self.assertEqual(cc.plan(cc.check_units(self.root))[0], [])

    def test_all_ignores_cache(self):
        self.mark_all_passed()
        hit, miss = cc.plan(cc.check_units(self.root), use_cache=False)
        self.assertEqual((hit, len(miss)), ([], 4))


class FailClosed(Base):
    def test_missing_import_target(self):
        self.write('src/mid.bend', 'import Base\nimport ./gone.bend as G\n')
        with self.assertRaises(cc.CacheError):
            cc.check_units(self.root)

    def test_unknown_import_form(self):
        self.write('src/mid.bend', 'import Base\nimport (weird)\n')
        with self.assertRaises(cc.CacheError):
            cc.check_units(self.root)

    def test_import_escaping_checkout(self):
        self.write('src/mid.bend', 'import Base\nimport ../../x.bend as X\n')
        with self.assertRaises(cc.CacheError):
            cc.check_units(self.root)

    def test_symlinked_dependency(self):
        (self.root/'src/real.bend').write_text('import Base\n')
        (self.root/'src/link.bend').symlink_to('real.bend')
        self.write('src/mid.bend', 'import Base\nimport ./link.bend as X\n')
        with self.assertRaises(cc.CacheError):
            cc.check_units(self.root)

    def test_incomplete_tool_identity(self):
        real = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(real)
        with patch.object(real, '_module', return_value=type('M', (), {'toolchain': staticmethod(lambda: {'binary_sha256': ''})})()):
            with self.assertRaises(real.CacheError):
                real.tool_identity()

    def test_unavailable_compiler(self):
        real = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(real)
        def boom():
            raise ValueError('bend is not on PATH')
        with patch.object(real, '_module', return_value=type('M', (), {'toolchain': staticmethod(boom)})()):
            with self.assertRaises(real.CacheError):
                real.tool_identity()

    def test_no_inputs_has_no_key(self):
        with self.assertRaises(cc.CacheError):
            cc.unit_key('check', 'x', {}, TOOL)


class Entries(Base):
    def entry(self, name='src/other.bend'):
        k, body = cc.check_units(self.root, [name])[name]
        return k, body

    def test_damaged_entries_are_misses(self):
        k, body = self.entry()
        cc.record(k, body)
        path = cc.entry_path(self.cache, k)
        good = json.loads(path.read_text())
        for bad in ('', '{', '[]', json.dumps({**good, 'verdict': 'failed'}),
                    json.dumps({**good, 'key': 'f'*64}), json.dumps({**good, 'schema': 99}),
                    json.dumps({**good, 'body': {**good['body'], 'unit': 'src/mid.bend'}})):
            path.write_text(bad)
            self.assertIsNone(cc.lookup(k), bad)

    def test_entry_filed_under_another_key_is_a_miss(self):
        k, body = self.entry('src/other.bend')
        k2, _ = self.entry('src/leaf.bend')
        cc.record(k, body)
        dest = cc.entry_path(self.cache, k2)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(cc.entry_path(self.cache, k).read_text())
        self.assertIsNone(cc.lookup(k2))

    def test_record_refuses_mismatched_body(self):
        k, body = self.entry()
        with self.assertRaises(cc.CacheError):
            cc.record(k, {**body, 'unit': 'x'})

    def test_shared_hit_is_copied_local_and_validated(self):
        k, body = self.entry()
        cc.record(k, body, local=self.shared)
        self.assertEqual(cc.lookup(k, shared=[self.shared]), 'shared')
        self.assertEqual(cc.lookup(k, shared=[]), 'local')

    def test_forged_shared_entry_is_ignored(self):
        k, body = self.entry()
        path = cc.entry_path(self.shared, k)
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps({'schema': 1, 'key': k, 'verdict': 'passed', 'body': {'unit': 'forged'}}))
        self.assertIsNone(cc.lookup(k, shared=[self.shared]))

    def test_shared_write_needs_a_shared_cache(self):
        k, body = self.entry()
        with self.assertRaises(cc.CacheError):
            cc.record(k, body, shared_write=True, shared=[])

    def test_shared_not_written_by_default(self):
        k, body = self.entry()
        cc.record(k, body, shared=[self.shared])
        self.assertFalse(cc.entry_path(self.shared, k).exists())


class Cli(Base):
    def run_cli(self, *args):
        return cc.main(['--root', str(self.root), *args])

    def test_record_refuses_inputs_edited_during_the_check(self):
        plan = self.root.parent/'plan.json'
        self.run_cli('plan-check', '--plan', str(plan))
        self.write('src/other.bend', 'import Base\ndef other() -> U32:\n  99\n')
        with self.assertRaises(cc.CacheError):
            self.run_cli('record-check', 'src/other.bend', '--plan', str(plan))
        self.assertIsNone(cc.lookup(self.keys(['src/other.bend'])['src/other.bend']))

    def test_record_refuses_unplanned_module(self):
        plan = self.root.parent/'plan.json'
        self.run_cli('plan-check', '--plan', str(plan))
        self.write('src/new.bend', 'import Base\n')
        with self.assertRaises(cc.CacheError):
            self.run_cli('record-check', 'src/new.bend', '--plan', str(plan))

    def test_record_needs_a_plan(self):
        with self.assertRaises(cc.CacheError):
            self.run_cli('record-check', 'src/other.bend')

    def test_roundtrip_and_audit(self):
        plan = self.root.parent/'plan.json'
        self.run_cli('plan-check', '--plan', str(plan))
        self.run_cli('record-check', 'src/other.bend', 'src/leaf.bend', '--plan', str(plan))
        hit, miss = cc.plan(cc.check_units(self.root))
        self.assertEqual(sorted(hit), ['src/leaf.bend', 'src/other.bend'])
        self.assertEqual(self.run_cli('audit'), 0)
        next(self.cache.glob('*/*.json')).write_text('{')
        self.assertEqual(self.run_cli('audit'), 1)


class Shards(unittest.TestCase):
    """Proof-shard keys follow the canonical partition and fail closed."""

    def setUp(self):
        import copy, types
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        self.root = Path(t.name)
        (self.root/'src').mkdir()
        self.write('src/unused.bend', 'import Base\ndef unused() -> U32:\n  0\n')
        self.write('LAWS.bend', 'import Base\nimport ./src/unused.bend as U\n\nlaw first:\n  {0n == 0n : Nat}\n\nlaw second:\n  {1n == 1n : Nat}\n')
        self.write('PROOF.bend', 'import Base\nimport ./LAWS.bend as Laws\n\ndef Laws.first():\n  {==}\n\ndef Laws.second():\n  {==}\n')
        self.ps = cc._module('proof-shards.py')
        original = self.ps.module
        self.tc = {'binary_sha256': 'A', 'bundled': {}, 'named_libraries': {}}
        tool = {'toolchain': self.tc, 'env': {}, 'logic': {}}
        for target, fn in ((self.ps, lambda n: types.SimpleNamespace(toolchain=lambda: copy.deepcopy(self.tc)) if n == 'ci-packet.py' else original(n)),):
            p = patch.object(target, 'module', side_effect=fn)
            p.start()
            self.addCleanup(p.stop)
        p = patch.object(cc, '_module', return_value=self.ps)
        p.start()
        self.addCleanup(p.stop)
        p = patch.object(cc, 'tool_identity', side_effect=lambda: tool)
        p.start()
        self.addCleanup(p.stop)
        self.generate()

    def write(self, name, text):
        (self.root/name).write_text(text)

    def generate(self):
        import shutil
        self.folder = self.root/'build/shards'
        shutil.rmtree(self.folder, ignore_errors=True)
        manifest, files = self.ps.plan(self.root, self.folder, 2)
        self.folder.mkdir(parents=True)
        for n, body in files.items():
            (self.folder/n).parent.mkdir(exist_ok=True)
            (self.folder/n).write_bytes(body)
        (self.folder/'manifest.json').write_bytes(self.ps.encoded(manifest))

    def keys(self):
        return {n: k for n, (k, _) in cc.shard_units(self.root, self.folder)[0].items()}

    def test_stable_and_changes_with_a_dependency(self):
        before = self.keys()
        self.assertEqual(before, self.keys())
        self.write('src/unused.bend', 'import Base\ndef unused() -> U32:\n  7\n')
        with self.assertRaises(ValueError):  # stale partition: refuses, never a hit
            self.keys()
        self.generate()
        after = self.keys()
        self.assertEqual(set(before), set(after))
        self.assertTrue(any(before[n] != k for n, k in after.items()))  # every shard reading it changed

    def test_changed_proof_changes_only_its_shard_or_all_never_none(self):
        before = self.keys()
        self.write('PROOF.bend', self.root.joinpath('PROOF.bend').read_text().replace('def Laws.second():\n  {==}', 'def Laws.second():\n  {==}\n'))
        self.generate()
        self.assertNotEqual(before, self.keys())

    def test_compiler_change_refuses(self):
        self.tc['binary_sha256'] = 'B'
        with self.assertRaises(ValueError):
            cc.shard_units(self.root, self.folder)


class Proof(unittest.TestCase):
    """Shard receipts count only when they are real passes for this partition."""

    def test_receipt_rules(self):
        ps = cc._module('proof-shards.py')
        ok = {'state': 'passed', 'manifest_sha256': 'm', 'inputs_unchanged': True, 'exit': 0, 'output': 'All terms check.\n'}
        self.assertTrue(cc.receipt_passes(ok, 'm', ps))
        for bad in ({**ok, 'state': 'failed'}, {**ok, 'manifest_sha256': 'x'},
                    {**ok, 'inputs_unchanged': False}, {**ok, 'exit': 1},
                    {**ok, 'output': 'boom'}, {**ok, 'output': ''}):
            self.assertFalse(cc.receipt_passes(bad, 'm', ps), bad)


if __name__ == '__main__':
    unittest.main()
