"""Execute real install-completion IO definitions on Boat or a hosted runner.

The small harness retains the actual channel operations and JSON encoding;
native packaged acceptance remains a separate requirement.
"""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
DEFS = ('Self.post.go', 'Self.post', 'Upgrade.note', 'Upgrade.say',
        'Upgrade.done.k', 'Upgrade.done', 'Install.done', 'Install.said')


def definition(source, name):
    match = re.search(r'^def ' + re.escape(name) + r'\(', source, re.M)
    if match is None:
        raise ValueError('Missing production definition: ' + name)
    tail = source[match.start():]
    end = re.search(r'\n(?=def |type |effect |foreign )', tail)
    return tail[:end.start() if end else len(tail)].rstrip() + '\n'


DRIVER = r'''
def show(m: Maybe<&1, Msg>) -> String:
  match m:
    case Some{Announce{text}}: text
    case Some{KProbe{force}}: "probe:" ++ Bool.show(force)
    case Some{UpdateReady{force}}: "restart:" ++ Bool.show(force)
    case None{}: "closed"

def probe(ok: Bool, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        next : Maybe<&1, Msg> <- Chan.recv(Msg, inbox)
        C.Check.str("install success forces runtime refresh without restart", show(next), "probe:True")
    case False{}:
      do IO<Unit>:
        Chan.close(Msg, inbox)
        next : Maybe<&1, Msg> <- Chan.recv(Msg, inbox)
        C.Check.str("failed install emits no refresh or restart", show(next), "closed")

def main() -> IO(Unit):
  do IO<Unit>:
    raw : Result<&1, &1, U32 & String, String> <- IO.get_env("TEST_INSTALL_ERROR")
    +err : String = Result.default(&1, &1, U32 & String, String, raw, "")
    +inbox : Chan(Msg) <- Chan.new(Msg, 8)
    Install.said(err, inbox)
    notice : Maybe<&1, Msg> <- Chan.recv(Msg, inbox)
    +fields : List<&2, J.KV> = Bool.pick(List<&2, J.KV>, String.is_empty(err),
      [J.KV{"kicad", J.Str{"installed"}}, J.KV{"state", J.Str{"Backplane KiCad installed"}}],
      [J.KV{"kicad", J.Str{"failed"}}, J.KV{"error", J.Str{err}}])
    C.Check.str("install outcome notice preserves exact text", show(notice), Upgrade.note(fields))
    probe(String.is_empty(err), inbox)
'''


class Completion(unittest.TestCase):
    def test_actual_completion_and_restart_regression_control(self):
        source = (ROOT / 'src/server/server.bend').read_text()
        body = '\n'.join(definition(source, name) for name in DEFS)
        refresh = 'Self.post(inbox, KProbe{True{}})'
        self.assertEqual(definition(source, 'Install.done').count(refresh), 1)
        self.assertNotIn('Upgrade.done(', definition(source, 'Install.done'))
        prefix = ('import Base\nimport ../../src/core/json.bend as J\n'
                  'import ../../test/check.bend as C\n\n'
                  'type Msg is Data:\n  Announce{text: String}\n'
                  '  KProbe{force: Bool}\n  UpdateReady{force: Bool}\n\n')
        (ROOT / 'build').mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=ROOT / 'build', prefix='install-refresh-') as tmp:
            folder = Path(tmp)
            for label, definitions in [('production', body), ('old-restart-control', body.replace(
                    refresh, 'Upgrade.done(False{}, err, inbox)', 1))]:
                program, emitted = folder / (label + '.bend'), folder / (label + '.js')
                program.write_text(prefix + definitions + DRIVER)
                compiled = subprocess.run(['bend', str(program), '-o', str(emitted)],
                                          cwd=ROOT, capture_output=True, text=True, timeout=90)
                self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
                errors = [''] if label == 'old-restart-control' else [
                    '', 'failed C:\\backplane_kicad\\', 'failed D:\\new tools\\test\\',
                    'network error with literal \\b/\\n/\\t and 日本']
                for error in errors:
                    with self.subTest(definitions=label, error=error):
                        result = subprocess.run(['bun', str(emitted)], cwd=ROOT,
                                                env=dict(os.environ, TEST_INSTALL_ERROR=error),
                                                capture_output=True, text=True, timeout=20)
                        output = result.stdout + result.stderr
                        if label == 'old-restart-control':
                            self.assertRegex(output, r'(?m)^FAIL install success forces runtime refresh without restart')
                        else:
                            self.assertEqual(result.returncode, 0, output)
                            self.assertNotRegex(output, r'(?m)^(FAIL|Error:)')
                            self.assertEqual(len(re.findall(r'^ok ', output, re.M)), 2, output)


if __name__ == '__main__':
    unittest.main(verbosity=2)
