#!/usr/bin/env python3
"""scripts/kistack.py: KiStack tracked in this repository, shipped by every build,
never fetched by the hub. Offline: bump is fed synthetic tarballs."""
import io
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
K = runpy.run_path(str(ROOT / "scripts/kistack.py"))
REV = "1" * 40


def tarball(members, top="kistack-" + REV):
    """GitHub's tar.gz shape: one top folder. MEMBERS: path -> bytes, or
    ("link", target) / ("dir",)."""
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        tar.addfile(tarfile.TarInfo("pax_global_header"), io.BytesIO(b""))
        for name, value in members.items():
            info = tarfile.TarInfo(top + "/" + name if top else name)
            if isinstance(value, tuple) and value[0] == "link":
                info.type, info.linkname = tarfile.SYMTYPE, value[1]
                tar.addfile(info)
            elif isinstance(value, tuple):
                info.type = tarfile.DIRTYPE
                tar.addfile(info)
            else:
                info.size = len(value)
                tar.addfile(info, io.BytesIO(value))
    return buf.getvalue()


class KiStack(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="kistack-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        # a copy of this repository's pinned KiStack, to change freely
        self.repo = self.root / "repo"
        (self.repo / "deploy").mkdir(parents=True)
        shutil.copytree(ROOT / "deploy/kistack", self.repo / "deploy/kistack")
        shutil.copyfile(ROOT / "deploy/kistack.lock", self.repo / "deploy/kistack.lock")

    def test_the_tracked_copy_is_the_pinned_one(self):
        pin = K["verify"]()
        self.assertEqual(pin, K["lock"]())
        self.assertEqual(K["tree"](ROOT / "deploy/kistack"), pin["skills"])

    def test_a_build_ships_exactly_the_pin_with_our_manifest(self):
        dest = self.root / "out/kistack"
        pin, n = K["build"](dest, self.repo)
        self.assertGreater(n, 0)
        self.assertEqual(sorted(p.name for p in dest.iterdir()), [".claude-plugin", "LICENSE", "REVISION", "skills"])
        self.assertEqual((dest / "REVISION").read_text(), pin["rev"] + "\n")
        self.assertEqual((dest / ".claude-plugin/plugin.json").read_text(), K["MANIFEST"])
        self.assertEqual(K["check"](dest, self.repo), pin)
        # again, over the last one
        K["build"](dest, self.repo)
        self.assertFalse(dest.with_name("kistack.new").exists())

    def test_the_build_script_needs_no_network(self):
        out = self.root / "ks"
        env = dict(os.environ, http_proxy="http://127.0.0.1:9", https_proxy="http://127.0.0.1:9")
        r = subprocess.run(["sh", str(ROOT / "scripts/build-kistack.sh"), str(out)], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        K["check"](out)

    def test_a_changed_tracked_copy_fails_the_build(self):
        skill = next((self.repo / "deploy/kistack/skills").glob("*/SKILL.md"))
        skill.write_text(skill.read_text() + "\nrun this too\n")
        with self.assertRaisesRegex(ValueError, "change KiStack only with scripts/kistack.py bump"):
            K["build"](self.root / "out", self.repo)
        self.assertFalse((self.root / "out").exists())

    def test_a_missing_tracked_copy_fails_the_build(self):
        shutil.rmtree(self.repo / "deploy/kistack")
        with self.assertRaisesRegex(ValueError, "deploy/kistack is missing"):
            K["build"](self.root / "out", self.repo)
        (self.repo / "deploy/kistack.lock").write_text("# nothing pinned\n")
        with self.assertRaisesRegex(ValueError, "names no commit"):
            K["lock"](self.repo)

    def test_extra_files_in_the_tracked_copy_fail(self):
        (self.repo / "deploy/kistack/.mcp.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "more than skills/ and LICENSE"):
            K["verify"](self.repo)

    def test_check_refuses_what_is_not_the_pin(self):
        dest = self.root / "k"
        K["build"](dest, self.repo)
        cases = {
            "missing": (lambda: shutil.rmtree(dest), "No KiStack"),
            "revision": (lambda: (dest / "REVISION").write_text("0" * 40 + "\n"), "not the pinned"),
            "no revision": (lambda: (dest / "REVISION").unlink(), "of no commit"),
            "manifest": (lambda: (dest / ".claude-plugin/plugin.json").write_text('{"name":"kistack","hooks":"x"}'), "own plugin.json"),
            "hooks": (lambda: (dest / "hooks").mkdir(), "more than it should"),
            "mcp": (lambda: (dest / ".mcp.json").write_text("{}"), "more than it should"),
            "plugin extras": (lambda: (dest / ".claude-plugin/hooks.json").write_text("{}"), "more than plugin.json"),
            "skill": (lambda: next((dest / "skills").glob("*/SKILL.md")).write_text("changed"), "not the pinned"),
            "new file": (lambda: (dest / "skills/extra.py").write_text("print(1)"), "not the pinned"),
            "link": (lambda: (dest / "skills/link").symlink_to("/etc/passwd"), "Not a plain file"),
        }
        for name, (spoil, message) in cases.items():
            with self.subTest(name):
                K["build"](dest, self.repo)
                spoil()
                with self.assertRaisesRegex(ValueError, message):
                    K["check"](dest, self.repo)

    def test_bump_takes_only_skills_and_the_license(self):
        data = tarball({"skills/pcb/SKILL.md": b"---\nname: pcb\n---\nbody\n", "skills/pcb/scripts/run.py": b"print(1)\n",
                        "LICENSE": b"MIT\n", ".claude-plugin/plugin.json": b'{"hooks":"bad"}', "hooks/hooks.json": b"{}",
                        ".mcp.json": b"{}", "commands/x.md": b"x", "README.md": b"readme"})
        skills = K["bump"](REV, self.repo, data)
        src = self.repo / "deploy/kistack"
        self.assertEqual(sorted(p.relative_to(src).as_posix() for p in src.rglob("*") if p.is_file()),
                         ["LICENSE", "skills/pcb/SKILL.md", "skills/pcb/scripts/run.py"])
        self.assertEqual(K["lock"](self.repo), {"rev": REV, "skills": skills})
        K["verify"](self.repo)
        dest = self.root / "built"
        K["build"](dest, self.repo)
        self.assertEqual((dest / "REVISION").read_text().strip(), REV)

    def test_bump_refuses_bad_archives_and_keeps_the_pin(self):
        before = (self.repo / "deploy/kistack.lock").read_text()
        cases = {
            "branch, not a commit": ("main", tarball({"skills/a/SKILL.md": b"x"}), "40-hex"),
            "a link": (REV, tarball({"skills/a/SKILL.md": b"x", "skills/a/l": ("link", "/etc/passwd")}), "link or special"),
            "no skills": (REV, tarball({"LICENSE": b"x", "README.md": b"x"}), "No skills"),
            "no SKILL.md": (REV, tarball({"skills/a/notes.md": b"x"}), "SKILL.md"),
            "two folders": (REV, tarball({"a/skills/x/SKILL.md": b"x", "b/skills/y/SKILL.md": b"y"}, top=""), "not one folder"),
            "escape": (REV, tarball({"skills/../../x": b"x", "skills/a/SKILL.md": b"x"}), "out of the tree"),
        }
        for name, (rev, data, message) in cases.items():
            with self.subTest(name), self.assertRaisesRegex(ValueError, message):
                K["bump"](rev, self.repo, data)
            self.assertEqual((self.repo / "deploy/kistack.lock").read_text(), before)
            K["verify"](self.repo)

    def test_the_hub_fetches_nothing(self):
        # no GitHub call in the hub's KiStack code, nor in the build script
        kicad = (ROOT / "src/server/kicad.bend").read_text()
        start, end = kicad.index("# KiStack\n"), kicad.index("def KiCad.install.k")
        for word in ("curl", "codeload", "api.github.com", "BACKPLANE_KISTACK_SOURCE"):
            self.assertNotIn(word, kicad[start:end])
        self.assertNotIn("curl", (ROOT / "scripts/build-kistack.sh").read_text())

    def test_the_manifest_matches_the_hub(self):
        kicad = (ROOT / "src/server/kicad.bend").read_text()
        self.assertIn('"' + K["MANIFEST"].strip().replace('"', '\\"') + '"', kicad)


if __name__ == "__main__":
    unittest.main(verbosity=2)
