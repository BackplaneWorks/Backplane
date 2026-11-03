import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('proof_slice', ROOT/'scripts/proof-slice.py')
sl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sl)


class Slices(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)/'repo'
        (self.root/'src').mkdir(parents=True)

    def write(self, rel, text):
        (self.root/rel).write_text(text)

    def slicer(self, *rels):
        return sl.Slicer(self.root, rels)

    def cycle(self):
        self.write('src/a.bend', 'import Base\nimport ./b.bend as B\n\ndef f(n):\n  B.g(n)\n\ndef unused():\n  0\n')
        self.write('src/b.bend', 'import Base\nimport ./a.bend as A\n\ndef g(n):\n  A.f(n)\n')
        return self.slicer('src/a.bend', 'src/b.bend')

    # entry reachability -------------------------------------------------
    def test_import_cycle_still_reaches_every_kept_module(self):
        s = self.cycle()
        out = self.root/'out'
        kept = s.write_shard(s.blocks('src/a.bend', 'f'), out)
        self.assertEqual({k[0] for k in kept}, {'src/a.bend', 'src/b.bend'})
        entry = (out/'PROOF.bend').read_text()
        self.assertEqual(entry.count('import ./t/'), 1)      # one member of the component
        sl.verify_output(out, kept)                           # both modules reachable from it

    def test_entry_with_only_base_is_rejected(self):
        s = self.cycle()
        out = self.root/'out'
        kept = s.write_shard(s.blocks('src/a.bend', 'f'), out)
        (out/'PROOF.bend').write_text('import Base\n')
        with self.assertRaisesRegex(ValueError, 'unreachable'):
            sl.verify_output(out, kept)

    def test_scc_helper_groups_cycles_and_orders_components(self):
        comp = sl.scc({'a': {'b'}, 'b': {'a'}, 'c': {'a'}})
        self.assertEqual(comp['a'], comp['b'])
        self.assertNotEqual(comp['c'], comp['a'])

    # fail-closed grammar ------------------------------------------------
    def test_unknown_top_level_line_before_first_def_is_rejected(self):
        self.write('src/m.bend', 'import Base\nconst X = 1\ndef f():\n  0\n')
        with self.assertRaisesRegex(ValueError, 'unsupported top-level'):
            self.slicer('src/m.bend')

    def test_unknown_or_misplaced_top_level_lines_are_rejected(self):
        for text in ('import Base\ndef f():\n  0\nconst X = 1\n',
                     'import Base\ndef f():\n  0\nimport ./m.bend as M\n',
                     'import Base\n  stray indented\ndef f():\n  0\n',
                     'import Base\nimport ./m.bend as M\nimport ./m.bend as M\n',
                     'import Base\ndef f():\n  0\ndef f():\n  1\n',
                     'import Base\nextern f\n'):
            self.write('src/m.bend', text)
            with self.assertRaises(ValueError, msg=text):
                self.slicer('src/m.bend')

    def test_comments_blanks_and_supported_headers_are_accepted(self):
        self.write('src/m.bend', 'import Base\n# note\n\ntype T is Data:\n  A{}\n\nlaw l:\n  {0n == 0n : Nat}\n\ndef f():\n  A{}\n')
        s = self.slicer('src/m.bend')
        self.assertEqual({(b['kind'], b['name']) for b in s.module('src/m.bend').blocks}, {('type', 'T'), ('law', 'l'), ('def', 'f')})

    # independent output coverage ----------------------------------------
    def shard(self):
        s = self.cycle()
        out = self.root/'out'
        return s, out, s.write_shard(s.blocks('src/a.bend', 'f'), out)

    def test_dropped_or_extra_declaration_is_detected(self):
        s, out, kept = self.shard()
        path = out/'t/src/b.bend'
        path.write_text('import Base\nimport ./a.bend as A\n\n')
        with self.assertRaisesRegex(ValueError, 'differ'):
            sl.verify_output(out, kept)
        path.write_text(path.read_text() + 'def g(n):\n  A.f(n)\n\ndef extra():\n  0\n')
        with self.assertRaisesRegex(ValueError, 'differ'):
            sl.verify_output(out, kept)

    def test_unaccounted_line_and_bad_import_in_output_are_detected(self):
        s, out, kept = self.shard()
        path = out/'t/src/a.bend'
        original = path.read_text()
        path.write_text(original + '\nmystery line\n')
        with self.assertRaisesRegex(ValueError, 'unaccounted'):
            sl.verify_output(out, kept)
        path.write_text(original.replace('import ./b.bend as B', 'import ../../outside.bend as B'))
        with self.assertRaisesRegex(ValueError, 'does not resolve'):
            sl.verify_output(out, kept)

    def test_union_of_slices_equals_every_original_declaration(self):
        s = self.cycle()
        allb = s.all_declarations(['src/a.bend', 'src/b.bend'])
        kept = s.write_shard(s.blocks('src/a.bend', 'f'), self.root/'o1')
        self.assertNotEqual(kept, allb)                       # A.unused is not reached
        kept |= s.write_shard(s.blocks('src/a.bend', 'unused'), self.root/'o2')
        self.assertEqual(kept, allb)

    def test_callees_precede_callers_and_cycles_keep_original_order(self):
        self.write('src/m.bend', 'import Base\n\ndef m.top():\n  m.mid()\n\ndef m.mid():\n  m.low()\n\ndef m.low():\n  0\n\ndef m.p():\n  m.q()\n\ndef m.q():\n  m.p()\n')
        s = self.slicer('src/m.bend')
        out = self.root/'o'
        s.write_shard(s.blocks('src/m.bend', 'm.top') + s.blocks('src/m.bend', 'm.p'), out)
        names = [l.split()[1].split('(')[0] for l in (out/'t/src/m.bend').read_text().splitlines() if l.startswith('def ')]
        self.assertLess(names.index('m.low'), names.index('m.mid'))
        self.assertLess(names.index('m.mid'), names.index('m.top'))
        self.assertLess(names.index('m.p'), names.index('m.q'))

    def test_every_module_in_the_canonical_closure_parses_strictly(self):
        import subprocess
        import sys
        names = subprocess.check_output([sys.executable, str(ROOT/'scripts/bend-closure.py'), './LAWS.bend', './PROOF.bend'], cwd=ROOT, text=True).split()
        names = sorted(n for n in set(names) | {'LAWS.bend', 'PROOF.bend'} if n.endswith('.bend'))
        self.assertGreater(len(names), 300)
        for n in names:
            try:
                sl.Module(ROOT, n)
            except ValueError as error:
                self.fail(str(error))


if __name__ == '__main__':
    unittest.main()
