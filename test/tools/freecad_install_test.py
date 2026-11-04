"""Run the exact portable setup script with a tiny local AppImage stand-in."""
import json
import os
import signal
import time
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
ROOT = Path(__file__).resolve().parents[2]
SCRIPT = json.loads(re.search(r'def CadSpec.freecad.script\(\) -> String:\s*(".*")', (ROOT/'src/server/cad-install.bend').read_text()).group(1))

class Portable(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='bp-freecad-fixture-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.home = self.root/'hub 日本 data'
        self.pkg = self.root/'fixture.AppImage'
        self.pkg.write_text('''#!/bin/sh
set -eu
[ "$1" = --appimage-extract ]
mkdir -p squashfs-root/usr/bin
printf '#!/bin/sh\\nprintf "fixture console: %%s\\\\n" "$*"\\n' > squashfs-root/usr/bin/freecadcmd
cat > squashfs-root/AppRun <<'RUN'
#!/bin/sh
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cmd=$1; shift
exec "$here/usr/bin/$cmd" "$@"
RUN
chmod 700 squashfs-root/AppRun squashfs-root/usr/bin/freecadcmd
''')

    def run_install(self):
        return subprocess.run(['sh','-c',SCRIPT,'sh',str(self.home),str(self.pkg),'linux-x86_64','1.1.4'], capture_output=True,text=True,timeout=10)

    def test_extract_configure_and_command_pointer(self):
        r=self.run_install();self.assertEqual(r.returncode,0,r.stderr)
        command=(self.home/'freecad/command').read_text().strip()
        result=subprocess.run([command,'-c','fixture'],capture_output=True,text=True,timeout=5)
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn('fixture console:',result.stdout)
        self.assertIn('BP_INSTALL extract',r.stdout);self.assertIn('BP_INSTALL configure',r.stdout)
        self.assertIn('BP_COMMITTED',r.stdout)
        self.assertFalse(list((self.home/'freecad').glob('.install*')))

    def test_existing_install_preserved(self):
        self.assertEqual(self.run_install().returncode,0)
        before=(self.home/'freecad/command').read_bytes()
        r=self.run_install();self.assertNotEqual(r.returncode,0)
        self.assertIn('existing installation preserved',r.stderr)
        self.assertEqual((self.home/'freecad/command').read_bytes(),before)

    def test_cancel_extract_cleans_pointer_stage_and_lock(self):
        self.pkg.write_text('#!/bin/sh\nsleep 30\n')
        process = subprocess.Popen(['sh','-c',SCRIPT,'sh',str(self.home),str(self.pkg),'linux-x86_64','1.1.4'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            self.assertEqual(process.stdout.readline().strip(), 'BP_INSTALL extract')
            os.killpg(process.pid, signal.SIGTERM)
            _, _ = process.communicate(timeout=5)
            self.assertNotEqual(process.returncode, 0)
            self.assertFalse((self.home/'freecad/command').exists())
            self.assertFalse(list((self.home/'freecad').glob('.install*')))
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)

    def test_failed_extract_has_no_pointer_or_lock(self):
        self.pkg.write_text('#!/bin/sh\necho fixture-extraction-error >&2\nexit 7\n')
        r=self.run_install();self.assertNotEqual(r.returncode,0)
        self.assertIn('fixture-extraction-error',r.stderr)
        self.assertFalse((self.home/'freecad/command').exists())
        self.assertFalse(list((self.home/'freecad').glob('.install*')))

if __name__ == '__main__': unittest.main()
