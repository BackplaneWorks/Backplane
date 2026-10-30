"""Narrow source-review guards; no compiler, service, provider or UI runs."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("sidebar_draft_guard", ROOT / "scripts/sidebar-draft-guard.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class SidebarDraftGuard(unittest.TestCase):
    def copy_review(self, root):
        reviewed = json.loads((ROOT / guard.MANIFEST).read_text())
        for name in [*reviewed["files"], guard.MANIFEST, "scripts/bend-closure.py",
                     "scripts/sidebar-draft-guard.py", "scripts/build-js.sh"]:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, target)

    def inject_body_read(self, root):
        path = root / "src/web/view.bend"
        original = "  View.sidebar.frame.raw(lc, bots, u, plain, signals, U.Side.rows.ix.at(U.Ui.state(u), plain, U.Side.now(u), U.Side.cur.held(U.Ui.state(u), U.Ui.sel(u), U.Side.now(u))))"
        injected = '  D.El{"div", "", Nil{}, [D.Txt{U.Draft.of(u)}, ' + original.strip() + ']}'
        source = path.read_text()
        self.assertEqual(source.count(original), 1)
        path.write_text(source.replace(original, injected))
        return path

    def test_current_review_and_unknown_scratch_policy(self):
        receipt = guard.check(ROOT)
        self.assertEqual(receipt["state"], "passed")
        self.assertGreater(receipt["sourceFiles"], 100)
        self.assertIn('String.starts_with(key, "@")', (ROOT / "src/core/sidebar-drafts.bend").read_text())

    def test_body_read_in_sidebar_dependency_fails_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.copy_review(root)
            guard.check(root)
            changed = self.inject_body_read(root)
            before = changed.read_bytes()
            with self.assertRaisesRegex(guard.GuardError, "src/web/view.bend"):
                guard.check(root)
            self.assertEqual(changed.read_bytes(), before)

    def test_transitive_reader_missing_file_and_scope_drift_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.copy_review(root)
            reader = root / "src/core/client.bend"
            reader.write_text(reader.read_text() + '\n# changed transitively reachable draft reader\n')
            with self.assertRaisesRegex(guard.GuardError, "src/core/client.bend"):
                guard.check(root)
            self.copy_review(root)
            reader.unlink()
            with self.assertRaises(guard.GuardError):
                guard.check(root)
            self.copy_review(root)
            path = root / guard.MANIFEST
            reviewed = json.loads(path.read_text())
            del reviewed["files"]["src/core/client.bend"]
            path.write_text(json.dumps(reviewed))
            with self.assertRaisesRegex(guard.GuardError, "closure changed"):
                guard.check(root)

    def test_selected_mirror_bridge_source_change_fails_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.copy_review(root)
            guard.check(root)
            path = root / "src/server/thin-mirror.bend"
            path.write_text(path.read_text() + "\n# selected projection dependency changed\n")
            with self.assertRaisesRegex(guard.GuardError, "thin-mirror.bend"):
                guard.check(root)

    def test_native_bound_bridges_mutation_and_missing_pin_fail_closed(self):
        for name in ["src/core/native-sidebar.bend", "src/mobile/screen.bend",
                     "src/mobile/hubs.bend", "src/core/thin-wire.bend"]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                self.copy_review(root)
                guard.check(root)
                path = root / name
                path.write_text(path.read_text() + "\n# unreviewed native dependency change\n")
                with self.assertRaises(guard.GuardError):
                    guard.check(root)
                self.copy_review(root)
                manifest = root / guard.MANIFEST
                reviewed = json.loads(manifest.read_text())
                del reviewed["files"][name]
                manifest.write_text(json.dumps(reviewed))
                with self.assertRaises(guard.GuardError):
                    guard.check(root)

    def test_build_and_cached_key_reject_body_read_before_any_tool_or_output(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.copy_review(root)
            self.inject_body_read(root)
            bindir = root / "bin"
            bindir.mkdir()
            invoked = root / "compiler-was-invoked"
            for name in ["bend", "bun"]:
                tool = bindir / name
                tool.write_text('#!/bin/sh\ntouch "$GUARD_COMPILER_MARKER"\nexit 99\n')
                tool.chmod(0o755)
            env = {**os.environ, "PATH": str(bindir) + os.pathsep + os.environ["PATH"],
                   "GUARD_COMPILER_MARKER": str(invoked)}
            for args in [["--key", "src/server/main.bend"],
                         ["src/server/main.bend", "build/should-not-exist"]]:
                result = subprocess.run(["sh", "scripts/build-js.sh", *args], cwd=root,
                                        env=env, capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("sidebar-draft-guard:", result.stderr)
                self.assertIn("src/web/view.bend", result.stderr)
                self.assertEqual(result.stdout, "")
                self.assertFalse(invoked.exists())
                self.assertFalse((root / "build").exists())

    def test_loading_bridges_and_modules_fail_closed(self):
        for name in ["src/server/thin-body.bend", "src/core/thinq.bend", "src/server/thin-splice.bend", "src/server/thinstore.bend", "src/core/view-page.bend", "src/web/list-windows.bend"]:
            with self.subTest(path=name), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                self.copy_review(root)
                guard.check(root)
                path = root / name
                path.write_text(path.read_text() + "\n# changed loading dependency\n")
                with self.assertRaises(guard.GuardError):
                    guard.check(root)
                self.copy_review(root)
                manifest = root / guard.MANIFEST
                reviewed = json.loads(manifest.read_text())
                del reviewed["files"][name]
                manifest.write_text(json.dumps(reviewed))
                with self.assertRaisesRegex(guard.GuardError, "closure changed"):
                    guard.check(root)
                self.copy_review(root)
                (root / name).unlink()
                with self.assertRaises(guard.GuardError):
                    guard.check(root)

    def test_loading_bridge_scope_cannot_revert_silently(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.copy_review(root)
            manifest = root / guard.MANIFEST
            reviewed = json.loads(manifest.read_text())
            reviewed["bridges"].remove("src/core/thinq.bend")
            manifest.write_text(json.dumps(reviewed))
            with self.assertRaisesRegex(guard.GuardError, "scope changed"):
                guard.check(root)

    def test_canonical_retention_bridges_change_missing_pin_and_scope_fail_closed(self):
        for name in ["src/server/thin-signals.bend", "src/server/thin-signal-text.bend",
                     "src/server/thin-side-index.bend"]:
            with self.subTest(path=name), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                self.copy_review(root)
                guard.check(root)
                path = root / name
                path.write_text(path.read_text() + "\n# unreviewed canonical cache change\n")
                with self.assertRaises(guard.GuardError):
                    guard.check(root)
                self.copy_review(root)
                path.unlink()
                with self.assertRaises(guard.GuardError):
                    guard.check(root)
                self.copy_review(root)
                manifest = root / guard.MANIFEST
                reviewed = json.loads(manifest.read_text())
                del reviewed["files"][name]
                manifest.write_text(json.dumps(reviewed))
                with self.assertRaisesRegex(guard.GuardError, "closure changed"):
                    guard.check(root)
                self.copy_review(root)
                reviewed = json.loads(manifest.read_text())
                reviewed["bridges"].remove(name)
                manifest.write_text(json.dumps(reviewed))
                with self.assertRaisesRegex(guard.GuardError, "scope changed"):
                    guard.check(root)

    def test_required_local_gate_is_outside_optional_modes(self):
        source = (ROOT / "scripts/local-small.py").read_text()
        self.assertLess(source.index("'test/tools/sidebar_draft_guard_test.py'"),
                        source.index("if '--focused-only' not in sys.argv:"))


if __name__ == "__main__":
    unittest.main()
