"""The hub's git on a repository whose own config names commands (a folder
from a zip can ship its .git): run the production shell and environment on
disposable repositories, without a compiler, and check none of them runs."""
import ast
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
GIT = (ROOT / 'src/server/git.bend').read_text()
PROC = (ROOT / 'src/server/proc.bend').read_text()


def literal(source, name):
    return ast.literal_eval(source.split('def ' + name + '(', 1)[1].split('\ndef ', 1)[0].split('-> String:', 1)[1].strip())


def safe(name):
    text = GIT.split('def ' + name + '(', 1)[1].split('\ndef ', 1)[0].split('-> String:', 1)[1].strip()
    return literal(GIT, 'Git.safe.sh') + ast.literal_eval(text.split('Git.safe.sh() ++ ', 1)[1])


def hub_env():
    body = PROC.split('def Proc.git.env(', 1)[1].split('\ndef ', 1)[0]
    return dict(kv.split('=', 1) for kv in re.findall(r'"(GIT_[A-Z0-9_]+=[^"]*)"', body))


class GitRepoConfig(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='bp-git-config-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.mark = self.base / 'ran'
        self.env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1', GIT_AUTHOR_NAME='Synthetic',
                        GIT_AUTHOR_EMAIL='fixture@example.invalid', GIT_COMMITTER_NAME='Synthetic', GIT_COMMITTER_EMAIL='fixture@example.invalid')
        for key in list(self.env):
            if key.startswith('GIT_CONFIG_KEY_') or key.startswith('GIT_CONFIG_VALUE_') or key in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_CONFIG_COUNT'):
                del self.env[key]
        self.hub = dict(self.env, **hub_env())

    def git(self, repo, *args):
        return subprocess.run(['git', '-C', str(repo), *args], env=self.env, check=True, capture_output=True, text=True).stdout

    def repo(self, name, files):
        path = self.base / name
        path.mkdir()
        self.git(path, 'init', '-q', '-b', 'main')
        for rel, text in files.items():
            (path / rel).write_text(text)
        self.git(path, 'add', '-A')
        self.git(path, 'commit', '-qm', 'synthetic baseline')
        return path

    def run_hub(self, repo, script, *args):
        return subprocess.run(['sh', '-c', script, 'sh', *args], cwd=repo, env=self.hub, capture_output=True, text=True)

    def upstream(self, repo):
        return self.run_hub(repo, safe('Git.upstream.sh'), '0').stdout

    def test_upstream_fetch_skips_a_repository_upload_pack_command(self):
        source = self.repo('source', {'a.txt': 'a\n'})
        top = self.repo('top', {'b.txt': 'b\n'})
        self.git(top, 'remote', 'add', 'origin', str(source))
        self.git(top, 'config', 'remote.origin.uploadpack', 'touch "%s"; git-upload-pack' % self.mark)
        self.upstream(top)
        self.assertFalse(self.mark.exists())

    def test_upstream_fetch_skips_included_transport_commands(self):
        source = self.repo('source', {'a.txt': 'a\n'})
        top = self.repo('top', {'b.txt': 'b\n'})
        self.git(top, 'remote', 'add', 'origin', str(source))
        (top / '.git/extra').write_text('[core]\n\tsshCommand = touch "%s"\n' % self.mark)
        self.git(top, 'config', 'include.path', 'extra')
        self.git(top, 'config', 'remote.origin.url', 'ssh://example.invalid/x.git')
        self.upstream(top)
        self.assertFalse(self.mark.exists())

    def test_upstream_still_fetches_a_plain_origin(self):
        source = self.repo('source', {'a.txt': 'a\n'})
        top = self.repo('top', {'b.txt': 'b\n'})
        self.git(top, 'remote', 'add', 'origin', str(source))
        self.assertEqual(self.upstream(top), 'origin/main')

    def test_status_does_not_run_a_submodule_filter(self):
        sub = self.repo('sub', {'.gitattributes': '* filter=x\n', 'a.txt': 'a\n'})
        top = self.repo('top', {'b.txt': 'b\n'})
        subprocess.run(['git', '-C', str(top), '-c', 'protocol.file.allow=always', 'submodule', 'add', '-q', str(sub), 'lib'], env=self.env, check=True, capture_output=True)
        self.git(top, 'commit', '-qm', 'submodule')
        self.git(top / 'lib', 'config', 'filter.x.clean', 'touch "%s"; cat' % self.mark)
        os.utime(top / 'lib/a.txt', (1, 1))
        self.run_hub(top, literal(GIT, 'Git.safe.sh') + 'bp_safe; exec git "$@"', 'status', '--porcelain=v1', '-uall')
        self.assertFalse(self.mark.exists())

    def test_log_does_not_check_signatures_with_the_repository_program(self):
        top = self.repo('top', {'b.txt': 'b\n'})
        tree = self.git(top, 'write-tree').strip()
        head = self.git(top, 'rev-parse', 'HEAD').strip()
        body = ('tree %s\nparent %s\nauthor x <x@x> 1 +0000\ncommitter x <x@x> 1 +0000\n'
                'gpgsig -----BEGIN PGP SIGNATURE-----\n \n abc\n -----END PGP SIGNATURE-----\n\nsigned\n') % (tree, head)
        commit = subprocess.run(['git', '-C', str(top), 'hash-object', '-t', 'commit', '-w', '--stdin'], input=body, env=self.env, check=True, capture_output=True, text=True).stdout.strip()
        self.git(top, 'update-ref', 'refs/heads/main', commit)
        program = self.base / 'gpg'
        program.write_text('#!/bin/sh\ntouch "%s"\ncat >/dev/null\nexit 1\n' % self.mark)
        program.chmod(0o755)
        self.git(top, 'config', 'log.showSignature', 'true')
        self.git(top, 'config', 'gpg.program', str(program))
        self.run_hub(top, 'exec git "$@"', 'log', '-n', '2', '--format=%H %s')
        self.assertFalse(self.mark.exists())


if __name__ == '__main__':
    unittest.main()
