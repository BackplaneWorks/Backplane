"""Exercise actual palette-listing commands on disposable filenames."""
import ast
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class FileListRecordsTest(unittest.TestCase):
    def test_git_and_plain_folders_preserve_names(self):
        source = (ROOT / "src/server/filelist.bend").read_text()
        commands = re.findall(r'^      ("[^\n]+")$', source, re.M)
        self.assertEqual(len(commands), 2)
        names = ["café.step", "line\nbreak.step", "quote\".step", "back\\slash.step", " ", "\t"]
        for repo, encoded in [(True, commands[0]), (False, commands[1])]:
            with self.subTest(repo=repo), tempfile.TemporaryDirectory(prefix="bp-file-list-") as folder:
                root = Path(folder)
                for name in names:
                    (root / name).write_text("synthetic\n")
                if repo:
                    subprocess.run(["git", "init", "-q", folder], check=True)
                    subprocess.run(["git", "add", "--", names[0]], cwd=folder, check=True)
                result = subprocess.run(["sh", "-c", ast.literal_eval(encoded)], cwd=folder,
                                        capture_output=True, check=True, timeout=10)
                actual = result.stdout.decode().split("\0")
                self.assertEqual(actual.pop(), "")
                self.assertCountEqual(actual, names)


if __name__ == "__main__":
    unittest.main()
