"""Run the production PDF cache shell with a controlled Poppler stand-in."""
import ast
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class PdfCacheScriptTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="bp-pdf-cache-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "commands"
        self.bin.mkdir()
        self.pdf = self.root / "synthetic file.pdf"
        self.pdf.write_text("first PDF bytes")
        self.cache = self.root / "cache"
        source = (ROOT / "src/server/pdf.bend").read_text()
        self.script = "\n".join(ast.literal_eval(row.strip().removesuffix(",")) for row in source.splitlines()
                                if re.match(r'^    "', row))
        self.command("pdfinfo", "#!/bin/sh\necho 'Pages: 2'\n")
        self.command("pdftoppm", "#!/usr/bin/env python3\nimport os,sys\nfrom pathlib import Path\np=Path(sys.argv[-1]+'.png')\np.write_bytes(Path(sys.argv[-2]).read_bytes())\nraise SystemExit(int(os.environ.get('TEST_PDF_FAIL','0')))\n")

    def command(self, name, text):
        p = self.bin / name
        p.write_text(text)
        p.chmod(0o755)

    def render(self, fail=False):
        return subprocess.run(["sh", "-c", self.script, "sh", str(self.pdf), "1", str(self.cache)],
                              env={**os.environ, "PATH": str(self.bin) + os.pathsep + os.environ["PATH"],
                                   "TEST_PDF_FAIL": "1" if fail else "0"},
                              capture_output=True, text=True, timeout=5)

    def output(self, result):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        path, pages = result.stdout.split("\t")
        self.assertEqual(pages, "2")
        return Path(path)

    def test_same_size_same_timestamp_change_has_new_cache_identity(self):
        before = self.pdf.stat()
        first = self.output(self.render())
        self.pdf.write_text("other PDF bytes")
        os.utime(self.pdf, ns=(before.st_atime_ns, before.st_mtime_ns))
        second = self.output(self.render())
        self.assertNotEqual(first, second)
        self.assertEqual(first.read_bytes(), b"first PDF bytes")
        self.assertEqual(second.read_bytes(), b"other PDF bytes")

    def test_partial_failure_is_not_published_and_retry_succeeds(self):
        result = self.render(fail=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("could not draw page 1", result.stdout)
        self.assertEqual(list(self.cache.iterdir()), [])
        self.assertEqual(self.output(self.render()).read_bytes(), self.pdf.read_bytes())

    def test_empty_cache_is_rebuilt_and_successful_cache_is_reused(self):
        path = self.output(self.render())
        self.assertEqual(self.output(self.render(fail=True)), path)
        path.write_bytes(b"")
        self.assertNotEqual(self.render(fail=True).returncode, 0)
        self.assertEqual(self.output(self.render()).read_bytes(), self.pdf.read_bytes())

    def test_cache_directory_obstruction_is_preserved(self):
        self.cache.write_text("preserve")
        result = self.render()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("could not create PDF cache", result.stdout)
        self.assertEqual(self.cache.read_text(), "preserve")


if __name__ == "__main__":
    unittest.main()
