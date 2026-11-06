"""Real Git worktrees and byte changes through the production Bend resolver.

Run on Boat under scripts/heavy-boat.sh. TEST_KICAD_PROJECT_PROBE can name
an already emitted JS probe; otherwise this compiles the production imports.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class KiCadProject(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.output = tempfile.TemporaryDirectory(prefix="bp-kicad-project-emit-")
        cls.addClassCleanup(cls.output.cleanup)
        cls.probe = os.environ.get("TEST_KICAD_PROJECT_PROBE")
        if not cls.probe:
            cls.probe = str(Path(cls.output.name) / "probe.js")
            subprocess.run(["bend", "test/native/kicad_project.bend", "-o", cls.probe],
                           cwd=ROOT, check=True, timeout=180)

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="bp-kicad-main with spaces-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.main = self.base / "main"
        self.main.mkdir()
        self.git(self.main, "init", "-q")
        self.design(self.main)
        self.git(self.main, "add", ".")
        self.git(self.main, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "commit", "-qm", "fixture")
        self.thread = self.main / ".backplane/worktrees/thread"
        self.git(self.main, "worktree", "add", "--detach", "-q", str(self.thread))

    def git(self, cwd, *args):
        return subprocess.run(["git", "-C", str(cwd), *args], check=True,
                              capture_output=True, text=True, timeout=10).stdout.strip()

    def design(self, folder, name="board"):
        folder.mkdir(parents=True, exist_ok=True)
        for ext in ("kicad_pro", "kicad_pcb", "kicad_sch"):
            (folder / f"{name}.{ext}").write_text(f"fixture {name} {ext}\n")

    def resolve(self, cwd=None, **args):
        result = subprocess.run(["bun", self.probe], cwd=ROOT,
                                env={**os.environ, "TEST_PROJECT_ROOT": str(cwd or self.thread),
                                     "TEST_PROJECT_ARGS": json.dumps(args)},
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return json.loads(result.stdout)

    def test_worktree_defaults_to_main(self):
        result = self.resolve()
        self.assertEqual(result["main"]["project"], str(self.main / "board.kicad_pro"))
        self.assertEqual(result["request"]["path"], result["main"]["project"])
        self.assertFalse(result["changed"])
        self.assertFalse(result["prompt"])

    def test_modified_board_offers_choice_with_main_default(self):
        (self.thread / "board.kicad_pcb").write_text("modified thread board\n")
        result = self.resolve()
        self.assertTrue(result["changed"])
        self.assertTrue(result["prompt"])
        self.assertEqual(result["request"]["path"], str(self.main / "board.kicad_pro"))
        self.assertFalse(self.resolve(choice="main")["prompt"])
        self.assertFalse(self.resolve(choice="thread")["prompt"])

    def test_main_modified_since_fork_is_also_a_difference(self):
        (self.main / "board.kicad_sch").write_text("new main sheet\n")
        self.assertTrue(self.resolve()["prompt"])

    def test_deleted_board_is_a_difference(self):
        (self.thread / "board.kicad_pcb").unlink()
        self.assertTrue(self.resolve()["prompt"])

    def test_hierarchy_sheet_changes_additions_and_deletions(self):
        for mode in ("modify", "add", "delete"):
            with self.subTest(mode=mode):
                for folder in (self.main, self.thread):
                    (folder / "sheets").mkdir(exist_ok=True)
                    (folder / "sheets/child.kicad_sch").write_text("same\n")
                child = self.thread / "sheets/child.kicad_sch"
                if mode == "delete":
                    child.unlink()
                elif mode == "add":
                    (self.thread / "sheets/extra.kicad_sch").write_text("new\n")
                else:
                    child.write_text("changed\n")
                self.assertTrue(self.resolve()["prompt"])

    def test_main_configuration_wins_over_relative_thread_configuration(self):
        self.design(self.main / "boards/current", "current")
        (self.main / ".backplane.json").write_text(json.dumps({"project": "boards/current/current.kicad_pro"}))
        (self.thread / ".backplane.json").write_text(json.dumps({"project": "board.kicad_pro"}))
        result = self.resolve(kind="board", path=str(self.thread / "board.kicad_pcb"))
        self.assertEqual(result["request"]["path"], str(self.main / "boards/current/current.kicad_pcb"))

    def test_absolute_thread_configuration_override_is_preserved(self):
        other = self.base / "explicit design"
        self.design(other)
        (self.thread / ".backplane.json").write_text(json.dumps({"project": str(other / "board.kicad_pro")}))
        result = self.resolve(kind="board")
        self.assertEqual(result["request"]["path"], str(other / "board.kicad_pcb"))
        self.assertFalse(result["prompt"])

    def test_absolute_main_board_and_sheet_overrides_are_preserved(self):
        other = self.base / "explicit design"
        self.design(other)
        (self.main / ".backplane.json").write_text(json.dumps({"pcb": str(other / "board.kicad_pcb"),
                                                               "schematic": str(other / "board.kicad_sch")}))
        self.assertEqual(self.resolve(kind="board")["request"]["path"], str(other / "board.kicad_pcb"))
        self.assertEqual(self.resolve(kind="schematic")["request"]["path"], str(other / "board.kicad_sch"))

    def test_ipc_path_and_nested_document_paths_agree(self):
        path = str(self.thread / "board.kicad_pcb")
        result = self.resolve(path=path, message={"header": {"document": {"path": path}}, "label": "unchanged"})
        request = result["request"]
        self.assertEqual(request["path"], str(self.main / "board.kicad_pcb"))
        self.assertEqual(request["message"]["header"]["document"]["path"], request["path"])
        self.assertEqual(request["message"]["label"], "unchanged")

    def test_explicit_thread_copy_keeps_paths(self):
        path = str(self.thread / "board.kicad_pcb")
        result = self.resolve(path=path, thread_copy=True, message={"path": path})
        self.assertEqual(result["request"]["path"], path)
        self.assertEqual(result["request"]["message"]["path"], path)
        self.assertEqual(self.resolve(kind="board", thread_copy=True)["request"]["path"], path)
        self.assertEqual(self.resolve(path="board.kicad_pcb", thread_copy=True)["request"]["path"], path)

    def test_non_worktree_is_unchanged(self):
        result = self.resolve(self.main, kind="board")
        self.assertEqual(result["root"], str(self.main))
        self.assertFalse(result["prompt"])
        plain = self.base / "plain"
        self.design(plain)
        result = self.resolve(plain)
        self.assertEqual(result["main"]["project"], str(plain / "board.kicad_pro"))

    def test_directory_spelling_is_not_evidence_of_a_worktree(self):
        folder = self.main / ".backplane/worktrees/not-a-worktree"
        self.design(folder)
        result = self.resolve(folder)
        self.assertEqual(result["root"], str(folder))

    def test_external_worktree_and_nested_project(self):
        folder = self.base / "elsewhere"
        self.git(self.main, "worktree", "add", "--detach", "-q", str(folder))
        for root in (self.main, folder):
            self.design(root / "hardware")
        result = self.resolve(folder / "hardware")
        self.assertEqual(result["root"], str(self.main / "hardware"))
        self.assertEqual(result["request"]["path"], str(self.main / "hardware/board.kicad_pro"))

    def test_separate_git_directory(self):
        separate = self.base / "separate"
        self.git(self.base, "init", "-q", "--separate-git-dir=" + str(self.base / "metadata"), str(separate))
        self.git(separate, "config", "core.worktree", str(separate))
        self.design(separate)
        self.git(separate, "add", ".")
        self.git(separate, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
        linked = self.base / "linked separate"
        self.git(separate, "worktree", "add", "--detach", "-q", str(linked))
        self.assertEqual(self.resolve(linked)["root"], str(separate))

    def test_missing_main_checkout_fails_closed(self):
        self.git(self.main, "config", "core.worktree", str(self.base / "missing"))
        result = self.resolve()
        self.assertEqual(result["root"], "")
        self.assertEqual(result["main"]["project"], "")
        self.assertEqual(result["request"]["path"], "")

    def test_divergent_ipc_writes_require_explicit_target(self):
        (self.thread / "board.kicad_pcb").write_text("29x22 real worktree design\n")
        (self.main / "board.kicad_pcb").write_text("70x50 stale main draft\n")
        for command in ("BeginCommit", "CreateItems", "UpdateItems", "DeleteItems", "EndCommit", "SaveDocument",
                        "SaveDocumentAs", "OpenDocument", "RefillZones", "SketchRoute", "UnknownFutureWrite", "GetAndDeleteItems"):
            with self.subTest(command=command):
                result = self.resolve(path=str(self.thread / "board.kicad_pcb"), type="kiapi.common.commands." + command)
                error = result["write_error"]
                self.assertIn("write refused", error)
                self.assertIn(str(self.main / "board.kicad_pcb"), error)
                self.assertIn(str(self.thread / "board.kicad_pcb"), error)
                self.assertIn("thread_copy:true", error)
                self.assertIn("main:true", error)

    def test_divergent_write_allows_each_explicit_target(self):
        (self.thread / "board.kicad_pcb").write_text("modified\n")
        for choice, expected in (({"thread_copy": True}, self.thread), ({"main": True}, self.main)):
            with self.subTest(choice=choice):
                result = self.resolve(path=str(self.thread / "board.kicad_pcb"), type="kiapi.common.commands.SaveDocument", **choice)
                self.assertEqual(result["write_error"], "")
                self.assertEqual(result["request"]["path"], str(expected / "board.kicad_pcb"))

    def test_identical_copies_and_non_worktree_writes_are_unchanged(self):
        self.assertEqual(self.resolve(type="kiapi.common.commands.BeginCommit")["write_error"], "")
        self.assertEqual(self.resolve(self.main, type="kiapi.common.commands.SaveDocument")["write_error"], "")

    def test_divergent_reads_still_default_to_main(self):
        (self.thread / "board.kicad_sch").write_text("modified\n")
        for command in ("kiapi.common.commands.GetItems", "kiapi.common.commands.GetVersion", "kiapi.backplane.GetCapabilities",
                        "kiapi.schematic.commands.GetSchematicHierarchy", "kiapi.common.commands.GetDocumentModifiedState"):
            with self.subTest(command=command):
                result = self.resolve(type=command)
                self.assertEqual(result["write_error"], "")
                self.assertEqual(result["request"]["path"], str(self.main / "board.kicad_pro"))

    def test_conflicting_explicit_targets_are_rejected(self):
        result = self.resolve(type="kiapi.common.commands.SaveDocument", main=True, thread_copy=True)
        self.assertIn("not both", result["write_error"])

    def test_selected_alternate_board_is_guarded_too(self):
        for root in (self.main, self.thread):
            (root / "other.kicad_pcb").write_text("same\n")
        (self.thread / "other.kicad_pcb").write_text("modified alternate\n")
        result = self.resolve(path=str(self.thread / "other.kicad_pcb"), type="kiapi.common.commands.SaveDocument")
        self.assertIn(str(self.main / "other.kicad_pcb"), result["write_error"])
        self.assertIn(str(self.thread / "other.kicad_pcb"), result["write_error"])

    def test_other_files_are_not_redirected(self):
        path = str(self.thread / "mech/housing.step")
        self.assertEqual(self.resolve(path=path, kind="mech")["request"]["path"], path)


if __name__ == "__main__":
    unittest.main(verbosity=2)
