"""Private file-only tests of pairing URL handlers; never runs an installer."""
import importlib.util
import plistlib
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
def load(name, relative):
    spec=importlib.util.spec_from_file_location(name,ROOT/relative)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module
local=load('pair_local','scripts/local.py')
release=load('pair_release','desktop/release-files.py')
class PairingPackaging(unittest.TestCase):
    def test_url_mime_stays_in_main_section_and_actions_are_preserved(self):
        lines=['[Desktop Entry]','Name=Backplane','Exec="/tmp/app"','Actions=New;','[Desktop Action New]','Name=New','Exec="/tmp/app" --new']
        result=local.desktop_lines(lines,'/tmp/icon.png')
        boundary=result.index('[Desktop Action New]')
        self.assertIn('MimeType=x-scheme-handler/backplane;',result[:boundary])
        self.assertIn('Exec="/tmp/app" %u',result[:boundary])
        self.assertEqual(result[boundary:],lines[4:])
        self.assertEqual(local.desktop_lines(result,'/tmp/icon.png'),result)
    def test_existing_mime_types_and_url_arguments_are_kept(self):
        lines=['[Desktop Entry]','Exec=/tmp/app %U','MimeType=text/plain;']
        result=local.desktop_lines(lines,'/tmp/icon.png')
        self.assertIn('MimeType=text/plain;x-scheme-handler/backplane;',result)
        self.assertIn('Exec=/tmp/app %U',result)
    def test_macos_bundle_registers_the_pairing_scheme(self):
        with tempfile.TemporaryDirectory() as temp:
            file=Path(temp)/'Info.plist';file.write_bytes(plistlib.dumps({'Unrelated':'kept'}))
            release.plist(file,'dev.backplane.desktop','0.13.0')
            data=plistlib.loads(file.read_bytes())
            self.assertEqual(data['Unrelated'],'kept')
            self.assertEqual(data['CFBundleURLTypes'][0]['CFBundleURLSchemes'],['backplane'])
if __name__=='__main__':unittest.main()
