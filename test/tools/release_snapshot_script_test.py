"""Exercise release snapshot guards in disposable repositories, without CAD."""
import ast
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def script(name):
    text = (ROOT / "src/server/review.bend").read_text().split("def " + name + "(", 1)[1].split("\ndef ", 1)[0]
    rows = text.split("String.join([", 1)[1].split("\n  ],", 1)[0]
    result = "\n".join(ast.literal_eval(line.strip().removesuffix(",").split('], "\\n")', 1)[0])
                       for line in rows.splitlines() if line.strip().startswith('"'))
    if "GT.Git.safe.sh() ++" in text:
        safe = (ROOT / "src/server/git.bend").read_text().split("def Git.safe.sh(", 1)[1].split("\ndef ", 1)[0]
        result = ast.literal_eval(safe.split("-> String:", 1)[1].strip()) + result
    return result


class ReleaseSnapshotScriptTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="bp-release-snapshot-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.scratch = self.root / "tmp"
        self.scratch.mkdir()
        self.env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1", "TMPDIR": str(self.scratch)}
        for key in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_CONFIG_COUNT"]:
            self.env.pop(key, None)
        self.git("init", "-q")
        self.git("config", "user.name", "Synthetic fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        (self.repo / "input.txt").write_text("committed input\n")
        self.git("add", "input.txt")
        self.git("commit", "-qm", "synthetic baseline")
        self.commit = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repo, env=self.env, text=True, stderr=subprocess.STDOUT)

    def run_guard(self, name, *args):
        return subprocess.run(["sh", "-c", script(name), "sh", *map(str, args)], cwd=self.repo,
                              env=self.env, capture_output=True, text=True, timeout=10)

    def test_source_refuses_uncommitted_and_staged_changes(self):
        result = self.run_guard("Rvr.source.sh", self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), self.commit)
        (self.repo / "input.txt").write_text("changed input\n")
        self.assertNotEqual(self.run_guard("Rvr.source.sh", self.repo).returncode, 0)
        self.git("add", "input.txt")
        self.assertNotEqual(self.run_guard("Rvr.source.sh", self.repo).returncode, 0)

    def test_tag_guard_refuses_moved_head(self):
        (self.repo / "input.txt").write_text("next input\n")
        self.git("commit", "-am", "next synthetic input")
        result = self.run_guard("Rvr.tag.sh", self.repo, "synthetic-v1", self.commit)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("HEAD moved", result.stdout)
        self.assertEqual(self.git("tag", "--list"), "")

    def test_prepared_snapshot_uses_commit_and_preserves_existing_outputs(self):
        result = self.run_guard("Rvr.tag.sh", self.repo, "synthetic-v1", self.commit)
        self.assertEqual(result.returncode, 0, result.stderr)
        (self.repo / "input.txt").write_text("later working copy\n")
        result = self.run_guard("Rvr.prep.sh", self.repo, "synthetic-v1", self.commit)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        fields = result.stdout.splitlines()
        self.assertEqual(len(fields), 11)
        self.assertEqual((Path(fields[1]) / "input.txt").read_text(), "committed input\n")
        self.assertEqual(fields[4], self.commit)
        # The guard transfers cleanup ownership to its caller on success.
        Path(fields[9]).rmdir()
        final = Path(fields[7])
        final.mkdir()
        (final / "preserve.txt").write_text("previous output")
        result = self.run_guard("Rvr.prep.sh", self.repo, "synthetic-v1", self.commit)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((final / "preserve.txt").read_text(), "previous output")


if __name__ == "__main__":
    unittest.main()
