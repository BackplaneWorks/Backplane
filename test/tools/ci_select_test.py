import hashlib
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('select_tests', ROOT/'scripts/ci-select.py')
selector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(selector)


class SelectionTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.write('src/leaf.bend', 'import Base\n')
        self.write('src/middle.bend', 'import ./leaf.bend as L\n')
        self.write('test/a_test.bend', 'import ../src/middle.bend as M\n')
        self.write('test/b_test.bend', 'import Base\n')
        self.write('test/required_test.bend', 'import Base\n')
        self.tests = ['test/a_test.bend', 'test/b_test.bend', 'test/required_test.bend']
        self.required = ['test/required_test.bend']

    def write(self, name, text):
        p = self.root/name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def plan(self, *changed):
        return selector.plan(self.root, self.tests, self.required, changed, {'File.open', 'Process.run'})

    def test_transitive_dependency_and_required_are_retained(self):
        p = self.plan('src/leaf.bend')
        self.assertEqual(p['selected'], ['test/a_test.bend', 'test/required_test.bend'])
        self.assertEqual(p['skipped'], ['test/b_test.bend'])

    def test_import_mutation_cannot_hide_new_dependency(self):
        before = self.plan('src/leaf.bend')
        self.write('test/b_test.bend', 'import ../src/leaf.bend as L\n')
        after = self.plan('src/leaf.bend')
        self.assertIn('test/b_test.bend', after['selected'])
        with self.assertRaisesRegex(ValueError, 'refusing to skip'):
            selector.validate(before, self.root, {'File.open', 'Process.run'})

    def test_every_changed_graph_node_selects_its_actual_dependents(self):
        # Independent fixture oracle: each test imports node i, whose chain is
        # i -> i-1 -> ... -> 0. A deliberately omitted affected root must fail.
        for name in self.tests:
            (self.root/name).unlink()
        tests = []
        for i in range(16):
            self.write(f'src/n{i}.bend', f'import ./n{i-1}.bend as Prev\n' if i else 'import Base\n')
            name = f'test/n{i}_test.bend'
            self.write(name, f'import ../src/n{i}.bend as Node\n')
            tests.append(name)
        for changed in range(16):
            with self.subTest(changed=changed):
                p = selector.plan(self.root, tests, [], [f'src/n{changed}.bend'], set())
                expected = {f'test/n{i}_test.bend' for i in range(changed, 16)}
                self.assertEqual(set(p['selected']), expected)
                p['selected'].remove(f'test/n{changed}_test.bend')
                with self.assertRaisesRegex(ValueError, 'refusing to skip'):
                    selector.validate(p, self.root, set())

    def test_added_test_invalidates_an_older_inventory(self):
        p = self.plan('src/leaf.bend')
        self.write('test/new_test.bend', 'import ../src/leaf.bend as L\n')
        with self.assertRaisesRegex(ValueError, 'inventory changed'):
            selector.validate(p, self.root, {'File.open'})
        with self.assertRaisesRegex(ValueError, 'invalid test inventory'):
            self.plan('src/leaf.bend')

    def test_failed_cli_cannot_leave_an_old_skip_list(self):
        out = self.root/'old-plan.json'
        out.write_text('{"selected": []}')
        with patch.object(selector.sys, 'argv', ['ci-select', '--base', 'bad-ref', '--out', str(out)]), \
                patch.object(selector.subprocess, 'check_output', side_effect=subprocess.CalledProcessError(1, ['git'])), \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(selector.main(), 3)
        self.assertFalse(out.exists())

    def test_drivers_effects_and_dynamic_base_operations_are_opaque(self):
        for code, extra in [
                ('import Base\n', ('test/tools/b.mjs', 'dynamic driver')),
                ('import Base\n', ('test/tools/b_test.py', 'dynamic driver')),
                ('import ../src/effect.js as E\n', ('src/effect.js', 'opaque')),
                ('import Base\ndef f(): File.open("runtime-path", "r")\n', None),
                ('import Base\ndef f(): Process.run("bend", [], "", 0)\n', None),
                ('import UnknownLibrary as U\n', None),
                ('import Base as B\n', None)]:
            with self.subTest(code=code, extra=extra):
                for name in ['test/tools/b.mjs', 'test/tools/b_test.py']:
                    (self.root/name).unlink(missing_ok=True)
                self.write('test/b_test.bend', code)
                if extra:
                    self.write(*extra)
                self.assertIn('test/b_test.bend', self.plan('src/leaf.bend')['selected'])

    def test_unknown_input_missing_dependency_and_syntax_select_all(self):
        self.assertEqual(self.plan('scripts/test.sh')['selected'], sorted(self.tests))
        self.write('test/b_test.bend', 'import ../src/missing.bend as M\n')
        self.assertEqual(self.plan('src/leaf.bend')['mode'], 'full')
        self.write('test/b_test.bend', 'import ./space name.bend as M\n')
        self.assertEqual(self.plan('src/leaf.bend')['selected'], sorted(self.tests))

    def test_symlinked_directory_cannot_hide_a_dependency_alias(self):
        (self.root/'linked').symlink_to(self.root/'src', target_is_directory=True)
        self.write('test/b_test.bend', 'import ../linked/leaf.bend as L\n')
        self.assertEqual(self.plan('src/leaf.bend')['mode'], 'full')

    def test_generated_transport_changes_keep_opaque_tests(self):
        self.write('test/b_test.bend', 'import Base\ndef f(): File.open("data", "r")\n')
        p = self.plan('.ci-packet/example.tar.gz')
        self.assertEqual(p['mode'], 'affected')
        self.assertIn('test/b_test.bend', p['selected'])
        self.assertIn('test/a_test.bend', p['skipped'])

    def test_invalid_required_or_negative_control_refuses_a_plan(self):
        for required, tests in [(['missing'], self.tests), ([], self.tests+[selector.NEGATIVE])]:
            with self.assertRaises(ValueError):
                selector.plan(self.root, tests, required, ['src/leaf.bend'], set())

    def test_supported_base_model_follows_foreign_wrappers(self):
        text = '''def IO.print(text):
  import "./effs/print.js"
def IO.get_env(name):
  import "./effs/get_env.js"
def File.open(path):
  import "./effs/file_open.js"
def Wrapped.read(path):
  File.open(path)
'''
        with patch.object(selector, 'BASE_SHA', hashlib.sha256(text.encode()).hexdigest()):
            opaque = selector.base_opaque(text)
        self.assertNotIn('IO.print', opaque)
        self.assertNotIn('IO.get_env', opaque)
        self.assertIn('File.open', opaque)
        self.assertIn('Wrapped.read', opaque)
        with self.assertRaisesRegex(ValueError, 'unsupported Base'):
            selector.base_opaque(text+'# new compiler\n')

    def test_custom_compiler_with_same_version_is_not_a_known_model(self):
        binary = self.root/'tool/bin/bend'
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b'custom compiler claiming2.0.36')
        with self.assertRaisesRegex(ValueError, 'unsupported compiler identity'):
            selector.compiler_model(binary)

    def test_changed_sidecar_effect_cannot_use_an_unchanged_base_model(self):
        binary = self.root/'tool/bin/bend'
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b'fixture compiler')
        lib = self.root/'tool/bend2'
        lib.mkdir()
        (lib/'base.bend').write_text('fixture Base')
        (lib/'print.js').write_text('original output effect')
        with patch.object(selector, 'BIN_SHA', {selector.file_digest(binary)}), \
                patch.object(selector, 'BUNDLED_SHA', selector.library_identity(lib)), \
                patch.object(selector, 'base_opaque', return_value=set()), \
                patch.dict(os.environ, {'BEND_LIB': str(self.root/'empty-libs')}):
            self.assertEqual(selector.compiler_model(binary), set())
            (lib/'print.js').write_text('different effect, same Base/compiler')
            with self.assertRaisesRegex(ValueError, 'unsupported compiler resources'):
                selector.compiler_model(binary)

    def test_diff_includes_deletions_untracked_and_both_rename_ends(self):
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        subprocess.run(['git', '-C', str(self.root), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.root), '-c', 'user.name=fixture', '-c',
                        'user.email=fixture@localhost', 'commit', '-qm', 'base'], check=True)
        (self.root/'src/leaf.bend').rename(self.root/'src/new.bend')
        self.write('test/new_test.bend', 'import Base\n')
        old = Path.cwd()
        try:
            os.chdir(self.root)
            changed = selector.changed_files('HEAD')
        finally:
            os.chdir(old)
        self.assertTrue({'src/leaf.bend', 'src/new.bend', 'test/new_test.bend'} <= changed)

    def test_git_bound_plan_cannot_hide_a_later_change(self):
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        subprocess.run(['git', '-C', str(self.root), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.root), '-c', 'user.name=fixture', '-c',
                        'user.email=fixture@localhost', 'commit', '-qm', 'base'], check=True)
        self.write('src/leaf.bend', 'import Base\n# first change\n')
        p = self.plan('src/leaf.bend')
        p['git_base'] = subprocess.check_output(['git', '-C', str(self.root), 'rev-parse', 'HEAD'], text=True).strip()
        selector.validate(p, self.root, {'File.open', 'Process.run'})
        self.write('test/b_test.bend', 'import Base\n# later change\n')
        with self.assertRaisesRegex(ValueError, 'refusing to skip'):
            selector.validate(p, self.root, {'File.open', 'Process.run'})


if __name__ == '__main__':
    unittest.main()
