"""python3 -m unittest discover -s test/tools -p laws_shape_test.py (repo root).

Synthetic broken LAWS/PROOF files for scripts/laws-shape.py, and a scripted
two-branch append through git with scripts/merge-laws.py as the merge driver.
"""
import importlib.util
import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def load(name, file):
    sp = importlib.util.spec_from_file_location(name, os.path.join(ROOT, 'scripts', file))
    m = importlib.util.module_from_spec(sp)
    sp.loader.exec_module(m)
    return m


SHAPE = load('shape', 'laws-shape.py')
MERGE = load('mergelaws', 'merge-laws.py')

LAWS = """import ./x.bend as X

# a
law a:
  {X.a() == 1 : Nat}

law b:
  for +n: Nat
  {X.b(n)
    == n : Nat}
"""
PROOF = """import ./LAWS.bend as Laws

def Laws.a():
  {==}

def Laws.b(n):
  X.b_lemma(n)
"""


class Shape(unittest.TestCase):
    def errs(self, laws=LAWS, proof=PROOF):
        return SHAPE.check(laws, proof)

    def test_good(self):
        self.assertEqual(self.errs(), [])

    def test_law_without_statement(self):
        e = self.errs(LAWS.replace('  {X.a() == 1 : Nat}\n', ''))
        self.assertTrue(any('law a has no' in x for x in e), e)

    def test_law_without_proof(self):
        e = self.errs(proof=PROOF.replace('def Laws.a():\n  {==}\n\n', ''))
        self.assertTrue(any('law a has no `def Laws.a`' in x for x in e), e)

    def test_orphan_proof(self):
        e = self.errs(proof=PROOF + '\ndef Laws.c():\n  {==}\n')
        self.assertTrue(any('Laws.c` has no law' in x for x in e), e)

    def test_proof_without_body(self):
        e = self.errs(proof=PROOF.replace('  {==}\n', ''))
        self.assertTrue(any('proof a has no body' in x for x in e), e)

    def test_proof_split_from_comment(self):
        e = self.errs(proof=PROOF.replace('  {==}\n', '# note\n'))
        self.assertTrue(any('proof a has no body' in x for x in e), e)

    def test_stray_statement(self):
        e = self.errs(proof=PROOF + '\n{==}\n')
        self.assertTrue(any('stray' in x for x in e), e)

    def test_conflict_marker(self):
        e = self.errs(LAWS + '<<<<<<< HEAD\n')
        self.assertTrue(any('conflict marker' in x for x in e), e)

    def test_duplicate(self):
        e = self.errs(LAWS + '\nlaw a:\n  {X.a() == 1 : Nat}\n')
        self.assertTrue(any('twice' in x for x in e), e)

    def test_real_files(self):
        lp, pp = (os.path.join(ROOT, f) for f in ('LAWS.bend', 'PROOF.bend'))
        self.assertEqual(SHAPE.main(['x', lp, pp]), 0)


class Merge(unittest.TestCase):
    def test_parse_roundtrip(self):
        for s in (LAWS, PROOF, LAWS + '\n# tail\n'):
            self.assertEqual(''.join(t for _, t in MERGE.parse(s)), s)

    def test_both_append(self):
        base = PROOF
        ours = base + '\ndef Laws.c():\n  {==}\n'
        theirs = base + '\ndef Laws.d():\n  {==}\n'
        out, conflict = MERGE.merge(base, ours, theirs)
        self.assertFalse(conflict)
        self.assertEqual(out.count('{==}'), 3)
        self.assertEqual(SHAPE.check(LAWS + 'law c:\n  {1 == 1 : Nat}\n\nlaw d:\n  {1 == 1 : Nat}\n', out), [])

    def test_same_block_conflict(self):
        out, conflict = MERGE.merge(PROOF, PROOF.replace('b_lemma', 'x'), PROOF.replace('b_lemma', 'y'))
        self.assertTrue(conflict)
        self.assertIn('<<<<<<<', out)

    def test_one_side_edits_other_adds(self):
        out, conflict = MERGE.merge(PROOF, PROOF.replace('b_lemma', 'x'), PROOF + '\ndef Laws.c():\n  {==}\n')
        self.assertFalse(conflict)
        self.assertIn('X.x(n)', out)
        self.assertIn('def Laws.c', out)

    def test_git_two_branch_append(self):
        with tempfile.TemporaryDirectory() as d:
            def git(*a):
                return subprocess.run(['git', '-C', d, '-c', 'user.name=t', '-c', 'user.email=t@t', *a],
                                      check=True, capture_output=True, text=True)

            git('init', '-q', '-b', 'main')
            git('config', 'merge.laws.driver',
                f"python3 {os.path.join(ROOT, 'scripts', 'merge-laws.py')} %O %A %B")
            open(os.path.join(d, '.gitattributes'), 'w').write('PROOF.bend merge=laws\n')
            put = lambda s: open(os.path.join(d, 'PROOF.bend'), 'w').write(s)
            put(PROOF)
            git('add', '.')
            git('commit', '-qm', 'base')
            git('checkout', '-qb', 'one')
            put(PROOF + '\ndef Laws.c():\n  {==}\n')
            git('commit', '-qam', 'one')
            git('checkout', '-q', 'main')
            git('checkout', '-qb', 'two')
            put(PROOF + '\ndef Laws.d():\n  Y.lemma()\n')
            git('commit', '-qam', 'two')
            git('merge', '-q', '--no-edit', 'one')
            out = open(os.path.join(d, 'PROOF.bend')).read()
            laws = LAWS + 'law c:\n  {1 == 1 : Nat}\n\nlaw d:\n  {1 == 1 : Nat}\n'
            self.assertEqual(SHAPE.check(laws, out), [], out)


if __name__ == '__main__':
    unittest.main()
