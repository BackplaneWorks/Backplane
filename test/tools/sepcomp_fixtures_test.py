import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('sepcomp', ROOT/'scripts/sepcomp-fixtures.py')
sc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sc)


class Fixtures(unittest.TestCase):
    def test_every_case_has_an_expectation_and_a_main(self):
        table = sc.expected()
        self.assertGreaterEqual(len(table), 15)
        self.assertEqual({v['verdict'] for v in table.values()}, {'pass', 'fail', 'unsafe-only'})

    def test_false_and_invalidation_cases_expect_rejection(self):
        table = sc.expected()
        for name in ('false_theorem', 'dependency_body_changed', 'body_replaced_cannot_convert', 'unfilled_law_unusable',
                     'import_cycle_rejected', 'mutual_recursion_rejected', 'forward_reference_rejected'):
            self.assertEqual(table[name]['verdict'], 'fail', name)
        self.assertEqual(table['conversion_ok']['verdict'], 'pass')

    def test_dependency_pairs_differ_only_in_the_dependency(self):
        base = ROOT/'test/separate-compilation/cases'
        self.assertEqual((base/'conversion_ok/B.bend').read_text(), (base/'dependency_body_changed/B.bend').read_text())
        self.assertNotEqual((base/'conversion_ok/A.bend').read_text(), (base/'dependency_body_changed/A.bend').read_text())

    def test_fill_order_pair_differs_only_in_the_entry(self):
        base = ROOT/'test/separate-compilation/cases'
        a, b = base/'filler_loaded_before_user_passes', base/'user_loaded_before_filler_fails'
        for name in ('L.bend', 'P.bend', 'U.bend'):
            self.assertEqual((a/name).read_text(), (b/name).read_text())
        self.assertNotEqual((a/'MAIN.bend').read_text(), (b/'MAIN.bend').read_text())
        self.assertNotIn('import ./P.bend', (a/'U.bend').read_text())     # U never imports the filler
        self.assertNotIn('P.bend', (base/'filler_never_loaded_fails/MAIN.bend').read_text())

    def test_classification_uses_the_repository_verdict(self):
        self.assertEqual(sc.classify(0, 'ALL PROOFS CHECK\n'), 'pass')
        self.assertEqual(sc.classify(1, 'SOME PROOFS FAIL\nError: 2 defs rely on unsafe or foreign code:\n- f\n- g\n'), 'unsafe-only')
        self.assertEqual(sc.classify(1, 'SOME PROOFS FAIL\nError:\n- expected : a filled definition\n'), 'fail')
        self.assertEqual(sc.classify(1, ''), 'fail')
        self.assertEqual(sc.classify(0, 'garbage'), 'fail')

    def test_run_refuses_without_a_verified_cap(self):
        with patch.object(sc, 'guard') as g, patch.object(sc.subprocess, 'run') as run:
            g.return_value.verify_shard_cgroup.return_value = False
            self.assertEqual(sc.run(), 75)
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
