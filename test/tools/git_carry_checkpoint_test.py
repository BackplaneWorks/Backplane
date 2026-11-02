"""Run the production shell helpers on disposable repositories, without a compiler."""
import ast
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / 'src/server/git.bend').read_text()

def body(name):
    text = SOURCE.split('def ' + name + '(', 1)[1].split('\ndef ', 1)[0]
    text = text.split('-> String:', 1)[1].strip()
    prefix = literal('Git.safe.sh') if text.startswith('Git.safe.sh() ++ ') else ''
    if 'String.join([' in text:
        rows = text.split('String.join([', 1)[1].split('\n  ],', 1)[0]
        return prefix + '\n'.join(ast.literal_eval(row.strip().removesuffix(',')) for row in rows.splitlines() if row.strip())
    return ast.literal_eval(text)

def literal(name):
    return ast.literal_eval(SOURCE.split('def ' + name + '(', 1)[1].split('\ndef ', 1)[0].split('-> String:', 1)[1].strip())

class GitCarryCheckpoint(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='bp-git-triage-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.repo = self.base / 'source'
        self.repo.mkdir()
        self.env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1')
        for key in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_CONFIG_COUNT'):
            self.env.pop(key, None)
        self.git('init', '-q')
        self.git('config', 'user.name', 'Synthetic fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        (self.repo / '.gitignore').write_text('ignored.txt\n')
        (self.repo / 'ignored.txt').write_text('committed\n')
        (self.repo / 'ordinary.txt').write_text('ordinary\n')
        self.git('add', '.gitignore', 'ordinary.txt')
        self.git('add', '-f', 'ignored.txt')
        self.git('commit', '-qm', 'synthetic baseline')

    def git(self, *args, repo=None, env=None):
        return subprocess.check_output(['git', '-C', str(repo or self.repo), *args], env=env or self.env, stderr=subprocess.STDOUT)

    def worktree(self, name):
        target = self.base / name
        self.git('worktree', 'add', '-q', '--detach', str(target), 'HEAD')
        return target

    def carry(self, source, target):
        subprocess.run(['sh', '-c', body('Git.carry.sh'), 'sh', str(source), str(target)], cwd=source, env=self.env, check=True, capture_output=True)

    def test_staged_addition_survives_repeated_carries(self):
        (self.repo / 'new board.step').write_text('synthetic new part\n')
        self.git('add', 'new board.step')
        first, second = self.worktree('first'), self.worktree('second')
        before = (self.repo / '.git/index').read_bytes()
        self.carry(self.repo, first)
        self.carry(first, second)
        self.assertEqual((second / 'new board.step').read_text(), 'synthetic new part\n')
        self.assertEqual((self.repo / '.git/index').read_bytes(), before)

    def test_staged_deletion_is_carried_without_source_index_changes(self):
        self.git('rm', '-q', 'ordinary.txt')
        dest = self.worktree('deleted')
        before = (self.repo / '.git/index').read_bytes()
        self.carry(self.repo, dest)
        self.assertFalse((dest / 'ordinary.txt').exists())
        self.assertEqual((self.repo / '.git/index').read_bytes(), before)

    def test_ignored_tracked_file_is_read_from_worktree_with_index_flags(self):
        self.git('update-index', '--assume-unchanged', 'ignored.txt')
        self.git('config', 'core.ignorestat', 'true')
        (self.repo / 'ignored.txt').write_text('current working content\n')
        before = (self.repo / '.git/index').read_bytes()
        index = self.repo / '.git/private-fixture.index'
        subprocess.run(['sh', '-c', body('Git.index.seed.sh'), 'sh', str(index)], cwd=self.repo, env=self.env, check=True, capture_output=True)
        env = dict(self.env, GIT_INDEX_FILE=str(index))
        self.git('add', '-A', env=env)
        tree = self.git('write-tree', env=env).decode().strip()
        self.assertEqual(self.git('show', tree + ':ignored.txt'), b'current working content\n')
        self.assertEqual((self.repo / '.git/index').read_bytes(), before)

    def test_untracked_ignored_file_is_not_carried(self):
        self.git('rm', '-q', '--cached', 'ignored.txt')
        self.git('commit', '-qm', 'stop tracking synthetic file')
        dest = self.worktree('ignored')
        self.carry(self.repo, dest)
        self.assertFalse((dest / 'ignored.txt').exists())

if __name__ == '__main__':
    unittest.main()
