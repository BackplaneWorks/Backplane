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
        sl.verify_output(out, kept, self.root)                           # both modules reachable from it

    def test_entry_with_only_base_is_rejected(self):
        s = self.cycle()
        out = self.root/'out'
        kept = s.write_shard(s.blocks('src/a.bend', 'f'), out)
        (out/'PROOF.bend').write_text('import Base\n')
        with self.assertRaisesRegex(ValueError, 'unreachable'):
            sl.verify_output(out, kept, self.root)

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
            sl.verify_output(out, kept, self.root)
        path.write_text(path.read_text() + 'def g(n):\n  A.f(n)\n\ndef extra():\n  0\n')
        with self.assertRaisesRegex(ValueError, 'differ'):
            sl.verify_output(out, kept, self.root)

    def test_unaccounted_line_and_bad_import_in_output_are_detected(self):
        s, out, kept = self.shard()
        path = out/'t/src/a.bend'
        original = path.read_text()
        path.write_text(original + '\nmystery line\n')
        with self.assertRaisesRegex(ValueError, 'unaccounted'):
            sl.verify_output(out, kept, self.root)
        path.write_text(original.replace('import ./b.bend as B', 'import ../../outside.bend as B'))
        with self.assertRaisesRegex(ValueError, 'not present in the original|does not resolve'):
            sl.verify_output(out, kept, self.root)

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

    # exact declaration text, imports, effects --------------------------------------
    def laws(self):
        self.write('src/l.bend', 'import Base\n\nlaw t:\n  {0n == 0n : Nat}\n\ndef Laws.t():\n  {==}\n')
        s = self.slicer('src/l.bend')
        out = self.root/'lo'
        return s, out, s.write_shard(s.blocks('src/l.bend', 't') + s.blocks('src/l.bend', 'Laws.t'), out)

    def test_same_name_statement_or_proof_change_is_detected(self):
        s, out, kept = self.laws()
        path = out/'t/src/l.bend'
        original = path.read_text()
        for old, new in (('{0n == 0n : Nat}', '{0n == 1n : Nat}'), ('{==}', '{=}'), ('def Laws.t():', 'def Laws.t(x):')):
            path.write_text(original.replace(old, new))
            with self.assertRaisesRegex(ValueError, 'differs from the original'):
                sl.verify_output(out, kept, self.root)
        path.write_text(original)
        sl.verify_output(out, kept, self.root)

    def test_duplicate_declaration_in_output_is_detected(self):
        s, out, kept = self.laws()
        path = out/'t/src/l.bend'
        path.write_text(path.read_text() + '\ndef Laws.t():\n  {==}\n')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            sl.verify_output(out, kept, self.root)

    def test_original_duplicate_declaration_is_rejected(self):
        self.write('src/d.bend', 'import Base\ndef f():\n  0\ndef f():\n  1\n')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            sl.scan_module((self.root/'src/d.bend').read_text(), 'd')

    def test_import_changes_are_detected(self):
        s, out, kept = self.shard()
        path = out/'t/src/a.bend'
        original = path.read_text()
        path.write_text(original.replace('import ./b.bend as B', 'import ./b.bend as Z'))
        with self.assertRaisesRegex(ValueError, 'not present in the original'):
            sl.verify_output(out, kept, self.root)
        path.write_text(original.replace('import ./b.bend as B\n', ''))
        with self.assertRaisesRegex(ValueError, 'no longer imports'):
            sl.verify_output(out, kept, self.root)

    def effects(self):
        (self.root/'src/effects').mkdir()
        self.write('src/effects/host.c', '#include "host.h"\nint x;\n')
        self.write('src/effects/host.h', 'int y;\n')
        self.write('src/e.bend', 'import Base\n\ndef E.now() -> U32:\n  import "./effects/host.c"\n  0\n\ndef E.other():\n  1\n')
        s = self.slicer('src/e.bend')
        out = self.root/'eo'
        return s, out, s.write_shard(s.blocks('src/e.bend', 'E.now'), out)

    def test_effect_companions_are_copied_with_their_headers_and_checked(self):
        s, out, kept = self.effects()
        self.assertTrue((out/'t/src/effects/host.c').is_file() and (out/'t/src/effects/host.h').is_file())
        sl.verify_output(out, kept, self.root)
        (out/'t/src/effects/host.h').write_text('int changed;\n')
        with self.assertRaisesRegex(ValueError, 'effect companion'):
            sl.verify_output(out, kept, self.root)
        (out/'t/src/effects/host.h').write_text('int y;\n')
        (out/'t/src/effects/host.c').unlink()
        with self.assertRaisesRegex(ValueError, 'effect companion'):
            sl.verify_output(out, kept, self.root)

    def test_extra_effect_file_and_missing_original_companion_are_rejected(self):
        s, out, kept = self.effects()
        (out/'t/src/effects/stray.js').write_text('x')
        with self.assertRaisesRegex(ValueError, 'effect companions do not match'):
            sl.verify_output(out, kept, self.root)
        (self.root/'src/effects/host.c').unlink()
        with self.assertRaisesRegex(ValueError, 'missing'):
            s.write_shard(s.blocks('src/e.bend', 'E.now'), self.root/'eo2')

    def test_real_indented_effect_import_matches_bend_closure(self):
        import subprocess
        import sys
        s = sl.Slicer(ROOT)
        rel = 'src/server/mailstore.bend'
        seed = next(b for b in s.module(rel).blocks if b['name'] == 'MailFile.open')
        out = self.root/'real'
        kept = s.write_shard([seed], out)
        copied = sorted(p.relative_to(out/'t').as_posix() for p in (out/'t').rglob('*') if p.is_file() and not p.name.endswith('.bend'))
        self.assertEqual(copied, ['src/server/effects/mailstore.js'])
        closure = subprocess.check_output([sys.executable, str(ROOT/'scripts/bend-closure.py'), 'PROOF.bend'], cwd=out, text=True).split()
        self.assertEqual(sorted(c[2:] for c in closure if c.startswith('t/') and not c.endswith('.bend')), copied)
        sl.verify_output(out, kept, ROOT)
        (out/'t'/copied[0]).write_bytes((out/'t'/copied[0]).read_bytes() + b' ')
        with self.assertRaisesRegex(ValueError, 'effect companion'):
            sl.verify_output(out, kept, ROOT)
        (out/'t'/copied[0]).unlink()
        with self.assertRaisesRegex(ValueError, 'effect companion'):
            sl.verify_output(out, kept, ROOT)

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
