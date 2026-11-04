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
        self.assertEqual((base/'conversion_ok/B.bend.txt').read_text(), (base/'dependency_body_changed/B.bend.txt').read_text())
        self.assertNotEqual((base/'conversion_ok/A.bend.txt').read_text(), (base/'dependency_body_changed/A.bend.txt').read_text())

    def test_fill_order_pair_differs_only_in_the_entry(self):
        base = ROOT/'test/separate-compilation/cases'
        a, b = base/'filler_loaded_before_user_passes', base/'user_loaded_before_filler_fails'
        for name in ('L.bend.txt', 'P.bend.txt', 'U.bend.txt'):
            self.assertEqual((a/name).read_text(), (b/name).read_text())
        self.assertNotEqual((a/'MAIN.bend.txt').read_text(), (b/'MAIN.bend.txt').read_text())
        self.assertNotIn('import ./P.bend', (a/'U.bend.txt').read_text())     # U never imports the filler
        self.assertNotIn('P.bend', (base/'filler_never_loaded_fails/MAIN.bend.txt').read_text())

    def test_classification_uses_the_repository_verdict(self):
        self.assertEqual(sc.classify(0, 'ALL PROOFS CHECK\n'), 'pass')
        self.assertEqual(sc.classify(1, 'SOME PROOFS FAIL\nError: 2 defs rely on unsafe or foreign code:\n- f\n- g\n'), 'unsafe-only')
        self.assertEqual(sc.classify(1, 'SOME PROOFS FAIL\nError:\n- expected : a filled definition\n'), 'fail')
        self.assertEqual(sc.classify(1, ''), 'fail')
        self.assertEqual(sc.classify(0, 'garbage'), 'fail')

    def run_with(self, lock, cgroup, admit):
        with patch.object(sc, 'guard') as g, patch.object(sc.subprocess, 'run') as run:
            g.return_value.inherited_lock.return_value = 7 if lock else None
            g.return_value.verify_shard_cgroup.return_value = cgroup
            g.return_value.admit.return_value = admit
            return sc.run(), run

    def test_run_refuses_unless_lock_cap_and_admission_all_hold(self):
        for lock, cgroup, admit in ((False, True, True), (True, False, True), (True, True, False), (False, False, False)):
            rc, run = self.run_with(lock, cgroup, admit)
            self.assertEqual(rc, 75, (lock, cgroup, admit))
            run.assert_not_called()

    def test_compiler_runs_with_telemetry_off_set_by_the_runner(self):
        seen = {}

        def fake(cmd, **kw):
            if cmd and cmd[0] == 'bend':
                seen['env'] = kw.get('env')
            return type('P', (), {'returncode': 0, 'stdout': 'ALL PROOFS CHECK\n', 'stderr': ''})()
        with patch.object(sc, 'guard') as g, patch.object(sc.subprocess, 'run', side_effect=fake), patch.dict(sc.os.environ, {}, clear=False):
            sc.os.environ.pop('BEND_NO_TELEMETRY', None)
            g.return_value.inherited_lock.return_value = 7
            g.return_value.verify_shard_cgroup.return_value = True
            g.return_value.admit.return_value = True
            g.return_value.own_cgroup.side_effect = RuntimeError('stop after the cases')
            with self.assertRaises(RuntimeError):
                sc.run()
        self.assertEqual(seen['env']['BEND_NO_TELEMETRY'], '1')

    def test_no_tracked_bend_files_are_ingested_by_check_inventories(self):
        import subprocess
        listed = subprocess.check_output(['git', 'ls-files', '-co', '--exclude-standard', '--', '*.bend'], cwd=ROOT, text=True).split()
        self.assertEqual([n for n in listed if 'separate-compilation' in n], [])

    def test_materialise_strips_the_suffix(self):
        import tempfile
        with tempfile.TemporaryDirectory() as work:
            main = sc.materialise('conversion_ok', work)
            self.assertTrue(main.is_file() and main.name == 'MAIN.bend')
            self.assertTrue((main.parent/'A.bend').is_file() and (main.parent/'B.bend').is_file())


if __name__ == '__main__':
    unittest.main()
