"""Admission routing only: all execs mocked, no compiler or real heavy lock."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('retained_guard', ROOT/'scripts/heavy-guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
ENV = {'BACKPLANE_PREEMITTED_JS': '/fixture/retained.js',
       'BACKPLANE_PREEMITTED_SHA256': 'a'*64, 'BACKPLANE_PREEMITTED_KEY': 'b'*40}
COMMAND = [str(ROOT/'scripts/build-js.sh'), 'src/server/main.bend', 'dist/service']


class RetainedContract(unittest.TestCase):
    def setUp(self):
        self.env = patch.dict(os.environ, ENV)
        self.env.start()
        self.addCleanup(self.env.stop)

    def run_guard(self, args, held=999, admitted=True):
        with patch.object(guard, 'inherited_lock', return_value=held), \
             patch.object(guard, 'admit', return_value=admitted), \
             patch.object(guard.os, 'set_inheritable'), \
             patch.object(guard.os, 'execvpe') as execute, \
             patch.object(guard, 'offload', return_value=75) as offload:
            status = guard.main(args)
        return status, execute, offload

    def test_arbitrary_commands_refused_before_admission_or_exec(self):
        for command in (['bend', 'PROOF.bend'], ['bun', 'build', '--compile', 'source.js'],
                        ['sh', '-c', 'anything'], ['python3', 'anything.py']):
            for held in (False, True):
                args = (['--held'] if held else []) + ['compile-js', '--', *command]
                with self.subTest(args=args), patch.object(guard, 'admit') as admit, \
                     patch.object(guard, 'inherited_lock') as lock, \
                     patch.object(guard.os, 'execvpe') as execute, \
                     patch.object(guard, 'offload') as offload:
                    self.assertEqual(guard.main(args), 2)
                    execute.assert_not_called()
                    offload.assert_not_called()
                    admit.assert_not_called()
                    lock.assert_not_called()

    def test_valid_entrypoint_and_held_command_keep_existing_admission(self):
        status, execute, offload = self.run_guard(['compile-js', '--', *COMMAND])
        self.assertIsNone(status)  # mocked exec returned; the real one replaces guard
        self.assertEqual(execute.call_args.args[:2], (COMMAND[0], COMMAND))
        offload.assert_not_called()
        for key_length in (40, 64):
            with patch.dict(os.environ, {'BACKPLANE_PREEMITTED_KEY': 'b'*key_length}):
                status, execute, _ = self.run_guard(['--held', 'compile-js', '--', *COMMAND])
                self.assertEqual(status, 0)
                execute.assert_not_called()
        status, execute, _ = self.run_guard(['--held', 'compile-js', '--', *COMMAND], held=None)
        self.assertEqual(status, 1)
        execute.assert_not_called()
        status, execute, _ = self.run_guard(['compile-js', '--', *COMMAND], admitted=False)
        self.assertEqual(status, 75)
        execute.assert_not_called()

    def test_partial_or_invalid_retained_environment_refused(self):
        for key in ENV:
            with self.subTest(key=key), patch.dict(os.environ, {key: ''}):
                status, execute, _ = self.run_guard(['compile-js', '--', *COMMAND])
                self.assertEqual(status, 2)
                execute.assert_not_called()
        for key, value in [('BACKPLANE_PREEMITTED_SHA256', 'bad'),
                           ('BACKPLANE_PREEMITTED_KEY', 'c'*39),
                           ('BACKPLANE_PREEMITTED_KEY', 'B'*40)]:
            with self.subTest(key=key, value=value), patch.dict(os.environ, {key: value}):
                status, execute, _ = self.run_guard(['compile-js', '--', *COMMAND])
                self.assertEqual(status, 2)
                execute.assert_not_called()

    def test_interpreter_wrappers_foreign_scripts_and_wrong_argv_refused(self):
        for command in (['sh', *COMMAND], ['/other/scripts/build-js.sh', *COMMAND[1:]],
                        COMMAND[:-1], COMMAND + ['extra'], [COMMAND[0], '--key', COMMAND[1]]):
            with self.subTest(command=command):
                status, execute, _ = self.run_guard(['compile-js', '--', *command])
                self.assertEqual(status, 2)
                execute.assert_not_called()
        status, execute, _ = self.run_guard(['--held', 'compile-js'])
        self.assertEqual(status, 2)
        execute.assert_not_called()

    def test_shell_helper_binds_current_entrypoint_and_exact_arguments(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'scripts').mkdir()
            probe = root/'scripts/heavy-guard.py'
            # This probe has no command execution; exercise real helper quoting.
            probe.write_text('import json,sys\nprint(json.dumps(sys.argv[1:]))\n')
            shell = '. "$1"; shift; heavy_guard compile-js "$@"'
            result = subprocess.run(['sh', '-c', shell, COMMAND[0],
                str(ROOT/'scripts/heavy-guard.sh'), 'entry with spaces.bend', 'dist/out with spaces'],
                cwd=root, text=True, capture_output=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), ['--held', 'compile-js', '--', COMMAND[0],
                'entry with spaces.bend', 'dist/out with spaces'])


if __name__ == '__main__':
    unittest.main()
