"""Exercise progress helpers only: no installer payload, download or install."""
import errno
import os
from pathlib import Path
import pty
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / 'deploy/install.sh.in').read_text()
HELPERS = SOURCE.split('# Progress helpers begin.', 1)[1].split('# Progress helpers end.', 1)[0]
# The marker's trailing sentence is a comment in the installer.
HELPERS = '#' + HELPERS


class Progress(unittest.TestCase):
    def run_helpers(self, body, flags=(), terminal=False, extra_env=None, shell='sh'):
        env = dict(os.environ, TERM='xterm', LC_ALL='C')
        env.pop('NO_COLOR', None)
        env.pop('BACKPLANE_QUIET', None)
        env.update(extra_env or {})
        with tempfile.TemporaryDirectory(prefix='bp-progress-helper-') as directory:
            env['HOME'] = directory
            script = 'set -eu\n' + HELPERS + '\n' + 'tmp="$HOME"\n' + body
            command = [shell, '-c', script, 'progress-fixture', *flags]
            if not terminal:
                result = subprocess.run(command, env=env, text=True, capture_output=True, timeout=5)
                return result.returncode, result.stdout, result.stderr
            master, slave = pty.openpty()
            try:
                proc = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=slave)
                os.close(slave)
                slave = None
                out, _ = proc.communicate(timeout=5)
                chunks = []
                while True:
                    try:
                        data = os.read(master, 4096)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not data:
                        break
                    chunks.append(data)
                return proc.returncode, out.decode(), b''.join(chunks).decode()
            finally:
                os.close(master)
                if slave is not None:
                    os.close(slave)

    def test_pipe_has_five_plain_result_lines(self):
        rc, out, err = self.run_helpers('for n in 1 2 3 4 5; do bp_begin "Step $n"; bp_done; done')
        self.assertEqual(rc, 0)
        self.assertEqual(out, '')
        self.assertEqual(err.splitlines(), [f'[{n}/5] Step {n}: done' for n in range(1, 6)])
        self.assertNotIn('\x1b', err)

    def test_terminal_feedback_and_harmless_extraction(self):
        rc, out, err = self.run_helpers('bp_begin Unpack; bp_extract printf fixture-output; bp_done; test -z "$bp_task_pid"', terminal=True)
        self.assertEqual(rc, 0)
        self.assertEqual(out, '')
        self.assertIn('fixture-output', err)
        self.assertIn('\x1b[2K', err)
        self.assertIn('Unpack: done', err)

    def test_quiet_and_no_color_disable_terminal_animation(self):
        for flags, env in [(('--quiet',), {}), (('-q',), {}), ((), {'BACKPLANE_QUIET': '1'}), ((), {'NO_COLOR': ''})]:
            with self.subTest(flags=flags, env=env):
                rc, _, err = self.run_helpers('bp_begin Test; bp_done', flags, True, env)
                self.assertEqual(rc, 0)
                self.assertNotIn('\x1b', err)
                self.assertIn('[1/5] Test: done', err)

    def test_failed_extraction_preserves_nonzero_and_reason(self):
        for terminal in (False, True):
            with self.subTest(terminal=terminal):
                rc, _, err = self.run_helpers('bp_begin Unpack; bp_extract sh -c "exit 7" || fail "unpack failed; not installed"; printf unexpected', terminal=terminal)
                self.assertEqual(rc, 1)
                self.assertIn('Unpack: failed', err)
                self.assertIn('unpack failed; not installed', err)

    def test_unknown_option_stops_before_any_step(self):
        rc, out, err = self.run_helpers('printf unexpected', ('--unknown',))
        self.assertEqual(rc, 2)
        self.assertEqual(out, '')
        self.assertIn('unknown option', err)
        self.assertNotIn('/5]', err)

    def test_posix_shell_syntax(self):
        for shell in ('sh', 'dash'):
            if shutil.which(shell):
                with self.subTest(shell=shell):
                    result = subprocess.run([shell, '-n', str(ROOT / 'deploy/install.sh.in')], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)



if __name__ == '__main__':
    unittest.main()
