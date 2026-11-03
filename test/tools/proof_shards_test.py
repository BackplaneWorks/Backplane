import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('proof_shards', ROOT/'scripts/proof-shards.py')
shards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shards)


class Partitions(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root/'src').mkdir()
        (self.root/'src/unused.bend').write_text('import Base\ndef unused() -> U32:\n  0\n')
        (self.root/'LAWS.bend').write_text('import Base\nimport ./src/unused.bend as U\n\nlaw first:\n  {0n == 0n : Nat}\n\nlaw second:\n  {1n == 1n : Nat}\n')
        (self.root/'PROOF.bend').write_text('import Base\nimport ./LAWS.bend as Laws\n\ndef helper() -> {1n == 1n : Nat}:\n  {==}\n\ndef Laws.first():\n  {==}\n\ndef Laws.second():\n  helper()\n')
        original = shards.module
        self.tool = {'binary_sha256':'fixed', 'bundled':{}, 'named_libraries':{}}
        patcher = patch.object(shards, 'module', side_effect=lambda n: types.SimpleNamespace(toolchain=lambda: copy.deepcopy(self.tool)) if n == 'ci-packet.py' else original(n))
        patcher.start()
        self.addCleanup(patcher.stop)
        self.folder = self.root/'build/shards'
        self.manifest, self.files = shards.plan(self.root, self.folder, 2)
        self.folder.mkdir(parents=True)
        for n, body in self.files.items():
            (self.folder/n).parent.mkdir(exist_ok=True)
            (self.folder/n).write_bytes(body)
        (self.folder/'manifest.json').write_bytes(shards.encoded(self.manifest))

    def test_exact_coverage_and_helpers(self):
        shards.verify(self.root, self.folder)
        original, *_ = shards.model(self.root)
        for worker in self.manifest['shards']:
            for origin in ['LAWS.bend', 'PROOF.bend']:
                _, copied = shards.parse((self.folder/worker['id']/origin).read_text(), origin)
                expected = [b for b in original[origin][1] if [origin,b['name']] in worker['declarations']]
                self.assertEqual(copied, expected)
        self.assertEqual(sorted(n for s in self.manifest['shards'] for n in s['primary_laws']), ['first','second'])
        for s in self.manifest['shards']:
            if 'second' in s['primary_laws']:
                self.assertIn(['PROOF.bend','helper'], s['declarations'])
        self.assertTrue(any('src/unused.bend' in s['source_files'] for s in self.manifest['shards']))
        # An imported Laws.X must not drag every Laws.* proof into each shard.
        self.assertTrue(all(sum(n.startswith('Laws.') for o,n in s['declarations']) == 1 for s in self.manifest['shards']))

    def test_dropped_real_law_fails_loudly(self):
        p = self.folder/'000/LAWS.bend'
        p.write_text('import Base\n')
        with self.assertRaises(ValueError):
            shards.verify(self.root, self.folder)

    def test_forged_partition_cannot_hide_law(self):
        m = copy.deepcopy(self.manifest)
        m['laws'].pop()
        (self.folder/'manifest.json').write_bytes(shards.encoded(m))
        with self.assertRaises(ValueError):
            shards.verify(self.root, self.folder)

    def test_unused_import_change_invalidates(self):
        (self.root/'src/unused.bend').write_text('import Base\ndef unused() -> U32:\n  "bad"\n')
        with self.assertRaises(ValueError):
            shards.verify(self.root, self.folder)

    def test_same_version_compiler_change_invalidates(self):
        self.tool['binary_sha256'] = 'changed'
        with self.assertRaises(ValueError):
            shards.verify(self.root, self.folder)

    def test_source_mode_change_invalidates(self):
        p = self.root/'src/unused.bend'
        p.chmod(p.stat().st_mode ^ 0o100)
        with self.assertRaises(ValueError):
            shards.verify(self.root, self.folder)

    def test_identity_ignores_umask_but_not_the_executable_bit(self):
        want = shards.encoded(self.manifest)
        for path in self.root.rglob('*'):
            if path.is_file() and path.parent != self.folder and 'shards' not in path.parts:
                path.chmod(0o600)          # Boat: umask 077
        manifest, _ = shards.plan(self.root, self.folder, 2)
        self.assertEqual(shards.encoded(manifest), want)
        self.assertEqual(set(manifest['source_modes'].values()), {0o644})
        shards.verify(self.root, self.folder)
        (self.root/'src/unused.bend').chmod(0o700)
        with self.assertRaises(ValueError):
            shards.verify(self.root, self.folder)

    def test_unknown_top_syntax_is_not_skipped(self):
        with self.assertRaises(ValueError):
            shards.parse('import Base\ntype New is Data:\n  New{}\n', 'future')

    def test_duplicate_law_is_rejected(self):
        p = self.root/'LAWS.bend'
        p.write_text(p.read_text()+'\nlaw first:\n  {0n == 0n : Nat}\n')
        with self.assertRaises(ValueError):
            shards.plan(self.root, self.folder, 2)

    def test_symlinked_import_parent_is_rejected(self):
        (self.root/'linked').symlink_to(self.root/'src', target_is_directory=True)
        p = self.root/'LAWS.bend'
        p.write_text(p.read_text().replace('./src/unused.bend','./linked/unused.bend'))
        with self.assertRaises(ValueError):
            shards.plan(self.root, self.folder, 2)

    def test_swap_availability_cannot_mask_paging(self):
        self.assertTrue(shards.no_swap_activity({'pswpin':3,'pswpout':7},{'pswpin':3,'pswpout':7}))
        self.assertFalse(shards.no_swap_activity({'pswpin':3,'pswpout':7},{'pswpin':3,'pswpout':8}))
        self.assertFalse(shards.no_swap_activity({},{}))

    def receipts(self):
        paths = []
        for s in self.manifest['shards']:
            p = self.root/(s['id']+'.json')
            p.write_text(json.dumps({'shard':s['id'],'manifest_sha256':shards.sha(shards.encoded(self.manifest)),
                'state':'passed','exit':0,'output':'ALL PROOFS CHECK\n','seconds':1.0,'peak_rss_kib':100,
                'swap_before':{'pswpin':0,'pswpout':0},'swap_after':{'pswpin':0,'pswpout':0},
                'memory_max_bytes':1024**3,'inputs_unchanged':True,'actual_bend_flock_held':True}))
            paths.append(p)
        return paths

    def test_join_requires_every_unique_receipt(self):
        ps = self.receipts()
        self.assertEqual(shards.join(self.manifest,self.folder,ps)['law_count'],2)
        for bad in [ps[:1], [ps[0],ps[0]]]:
            with self.assertRaises(ValueError):
                shards.join(self.manifest,self.folder,bad)

    def test_join_rejects_false_green_and_stale_source(self):
        for field, value in [('output','SOME PROOFS FAIL\nError: false theorem\n'),
                             ('manifest_sha256','old'),('inputs_unchanged',False),
                             ('peak_rss_kib',float('nan')),('swap_after',{'pswpin':0,'pswpout':1}),('memory_max_bytes',20*1024**3)]:
            ps = self.receipts()
            r = json.loads(ps[0].read_text());r[field] = value;ps[0].write_text(json.dumps(r))
            with self.assertRaises(ValueError):
                shards.join(self.manifest,self.folder,ps)


if __name__ == '__main__':
    unittest.main()
