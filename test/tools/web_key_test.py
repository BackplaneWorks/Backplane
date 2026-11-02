"""scripts/web-key.sh follows every Bend entry the web page bundles.

host.js imports timeline-plan.bend and timeline-history.bend beside
app.bend. Keyed by app.bend's imports alone, a change to the timeline's
planner (core/timeline-virtual.bend) or page history kept the key, and
scripts/dev.sh reused the cached client without the change.

  python3 -m unittest discover -s test/tools -p web_key_test.py
"""
import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class WebKeyRoots(unittest.TestCase):
    def roots(self):
        out = subprocess.run(["sh", "scripts/web-key.sh", "--roots"], cwd=ROOT, capture_output=True, text=True, check=True)
        return out.stdout.split()

    def test_every_imported_bend_entry_is_a_root(self):
        imported = set()
        for js in (ROOT / "src/web").glob("*.js"):
            for m in re.finditer(r"""(?:from|import\()\s*["']\./([A-Za-z0-9_-]+\.bend)["']""", js.read_text(encoding="utf-8")):
                imported.add("src/web/" + m.group(1))
        self.assertIn("src/web/timeline-plan.bend", imported)
        self.assertTrue(imported <= set(self.roots()), sorted(imported - set(self.roots())))

    def test_the_planner_is_in_the_keyed_files(self):
        out = subprocess.run(["python3", "scripts/bend-closure.py", *self.roots()], cwd=ROOT, capture_output=True, text=True, check=True)
        files = out.stdout.split()
        for f in ("src/core/timeline-virtual.bend", "src/web/timeline-history.bend", "src/web/app.bend"):
            self.assertIn(f, files)


if __name__ == "__main__":
    unittest.main()
