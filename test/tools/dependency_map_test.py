import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('dependency_map', ROOT/'scripts/dependency-map.py')
dm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dm)


class DependencyMap(unittest.TestCase):
    def test_closures_match_independent_walk_with_cycles_and_diamonds(self):
        edges = {0: {1, 2}, 1: {3}, 2: {3, 4}, 3: {1}, 4: set(), 5: set()}
        reached, components = dm.closures(edges, dm.load_slicer().scc)
        for node in edges:
            seen, pending = set(), [node]
            while pending:
                n = pending.pop()
                if n not in seen:
                    seen.add(n)
                    pending.extend(edges[n])
            self.assertEqual(reached[node], sum(1 << n for n in seen))
        self.assertEqual(components, [2, 1, 1, 1, 1])

    def test_law_counts_include_proof_dependencies_and_exclude_strings(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'LAWS.bend').write_text('import Base\nimport ./code.bend as C\nlaw small:\n  {C.f() == 0n : Nat}\n')
            (root/'PROOF.bend').write_text('import Base\nimport ./LAWS.bend as Laws\nimport ./code.bend as C\ndef Laws.small():\n  C.helper()\n')
            (root/'code.bend').write_text('import Base\ndef f():\n  0n\ndef helper():\n  "unused" # unused\ndef unused():\n  1n\n')
            result = dm.analyze(root, ['PROOF.bend'], include_graph=True)
            self.assertEqual(result['totals']['declarations'], 5)
            self.assertEqual(result['laws'][0]['declarations'], 4)
            self.assertEqual(result['laws'][0]['modules'], 3)
            code = next(m for m in result['modules'] if m['module'] == 'code.bend')
            self.assertEqual(code['direct_importers'], 2)
            self.assertEqual(code['transitive_importers'], 2)
            nodes = result['graph']['nodes']
            p = nodes.index(['PROOF.bend', 'def', 'Laws.small'])
            helper = nodes.index(['code.bend', 'def', 'helper'])
            self.assertIn(helper, result['graph']['edges'][p])
            self.assertEqual(set(result['sources']), {'LAWS.bend', 'PROOF.bend', 'code.bend'})
            first = result['source_identity']
            (root/'code.bend').write_text((root/'code.bend').read_text() + '# source change\n')
            self.assertNotEqual(first, dm.analyze(root, ['PROOF.bend'])['source_identity'])

    def test_missing_proof_fails_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'LAWS.bend').write_text('import Base\nlaw unproved:\n  {0n == 0n : Nat}\n')
            with self.assertRaisesRegex(ValueError, 'missing paired proof'):
                dm.analyze(root, ['LAWS.bend'])
