import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('retained', ROOT / 'scripts/retained-js.py')
retained = importlib.util.module_from_spec(spec)
spec.loader.exec_module(retained)
KEY = 'a' * 40


class RetainedJS(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.input = self.root / 'emit.js'
        self.input.write_text('console.log("exact producer bytes");\n')
        self.digest = hashlib.sha256(self.input.read_bytes()).hexdigest()

    def test_streamed_copy_is_exact(self):
        out = self.root / 'build/all.js'
        retained.consume(self.input, self.digest, KEY, KEY, out)
        self.assertEqual(out.read_bytes(), self.input.read_bytes())

    def test_same_size_corruption_preserves_prior_output(self):
        out = self.root / 'all.js'
        out.write_text('prior valid output')
        self.input.write_bytes(self.input.read_bytes().replace(b'exact', b'wrong'))
        with self.assertRaisesRegex(ValueError, 'bytes mismatch'):
            retained.consume(self.input, self.digest, KEY, KEY, out)
        self.assertEqual(out.read_text(), 'prior valid output')
        self.assertFalse(list(self.root.glob('.retained-*')))

    def test_closure_tool_drift_rejected(self):
        with self.assertRaisesRegex(ValueError, 'identity mismatch'):
            retained.consume(self.input, self.digest, KEY, 'b' * 40)

    def test_missing_input(self):
        self.input.unlink()
        with self.assertRaises(OSError):
            retained.consume(self.input, self.digest, KEY, KEY)

    def test_empty_input(self):
        self.input.write_text('')
        with self.assertRaisesRegex(ValueError, 'nonempty regular'):
            retained.consume(self.input, hashlib.sha256(b'').hexdigest(), KEY, KEY)

    def test_symlink_input(self):
        link = self.root / 'link.js'
        link.symlink_to(self.input)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            retained.consume(link, self.digest, KEY, KEY)

    def test_fifo_refused_without_waiting(self):
        fifo = self.root / 'fifo'
        os.mkfifo(fifo)
        with self.assertRaisesRegex(ValueError, 'regular'):
            retained.consume(fifo, self.digest, KEY, KEY)

    def test_incomplete_identity(self):
        for digest, key, actual in [('', KEY, KEY), (self.digest, '', ''), (self.digest, 'bad', 'bad')]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                retained.consume(self.input, digest, key, actual)


class BuildAdmission(unittest.TestCase):
    # Exercise the real shell entry point with an admission recorder. These are
    # routing/fail-closed controls, never performance or compiler qualification.
    def setUp(self):
        RetainedJS.setUp(self)
        (self.root / 'scripts').mkdir()
        for name in ['build-js.sh', 'retained-js.py']:
            shutil.copyfile(ROOT / 'scripts' / name, self.root / 'scripts' / name)
        (self.root / 'scripts/sidebar-draft-guard.py').write_text('')
        key = self.root / 'scripts/inputs-key.sh'
        key.write_text('#!/bin/sh\nprintf "%s\\n" ' + KEY + '\n')
        key.chmod(0o755)
        (self.root / 'scripts/heavy-guard.sh').write_text(
            'heavy_guard() { echo "$1" >> guard.log; '
            'if [ "${TEST_DRIFT:-}" = 1 ]; then printf changed > "$BACKPLANE_PREEMITTED_JS"; return; fi; exit 42; }\n')
        self.env = {**os.environ, 'BACKPLANE_PREEMITTED_JS': str(self.input),
                    'BACKPLANE_PREEMITTED_SHA256': self.digest, 'BACKPLANE_PREEMITTED_KEY': KEY,
                    'BACKPLANE_EMIT_CACHE': '0', 'PYTHONDONTWRITEBYTECODE': '1'}

    def run_build(self):
        return subprocess.run(['sh', 'scripts/build-js.sh', 'src/server/main.bend', 'dist/test'],
                              cwd=self.root, env=self.env, capture_output=True, text=True, timeout=5)

    def test_verified_input_selects_compile_profile(self):
        result = self.run_build()
        self.assertEqual(result.returncode, 42, result.stderr)
        self.assertEqual((self.root / 'guard.log').read_text(), 'compile-js\n')

    def test_ordinary_cache_cannot_select_compile_profile(self):
        for name in list(self.env):
            if name.startswith('BACKPLANE_PREEMITTED_'):
                del self.env[name]
        result = self.run_build()
        self.assertEqual(result.returncode, 42, result.stderr)
        self.assertEqual((self.root / 'guard.log').read_text(), 'build\n')

    def test_corrupt_retained_input_never_reaches_admission(self):
        self.input.write_text('bad producer bytes')
        result = self.run_build()
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertFalse((self.root / 'guard.log').exists())

    def test_source_key_drift_never_reaches_admission(self):
        self.env['BACKPLANE_PREEMITTED_KEY'] = 'b' * 40
        result = self.run_build()
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertFalse((self.root / 'guard.log').exists())

    def test_input_drift_after_admission_has_no_bend_fallback(self):
        # Supply a cheap Bun stand-in only to pass executable discovery. The
        # second retained-byte check must fail before any Bun/Bend invocation.
        bin_dir = self.root / 'bin'
        bin_dir.mkdir()
        for name in ['bun', 'bend']:
            tool = bin_dir / name
            tool.write_text('#!/bin/sh\necho invoked >> "' + str(self.root / 'compiler.log') + '"\nexit 99\n')
            tool.chmod(0o755)
        self.env.update(TEST_DRIFT='1', PATH=str(bin_dir) + os.pathsep + os.environ['PATH'])
        result = self.run_build()
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertFalse((self.root / 'compiler.log').exists())
        self.assertFalse((self.root / 'build/js/test/all.js').exists())


if __name__ == '__main__':
    unittest.main()
