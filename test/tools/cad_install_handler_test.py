"""New install controls must be reachable from Ui.act, with concrete RPCs."""
import importlib.util
from pathlib import Path
import re
import unittest
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('coverage', ROOT/'scripts/ahead-validation/coverage.py')
coverage = importlib.util.module_from_spec(spec); spec.loader.exec_module(coverage)

class Handlers(unittest.TestCase):
    def check_controls(self, client):
        defs = coverage.definitions(client)
        reachable = coverage.reachable(defs, ['Ui.act'])
        actions = set(re.findall(r'"(cad-[a-z]+)"', (ROOT/'src/core/cad-install.bend').read_text()))
        cases = set(re.findall(r'case "(cad-[a-z]+)":', reachable))
        return actions - cases

    def test_every_state_action_reaches_handler(self):
        client = (ROOT/'src/core/client.bend').read_text()
        self.assertEqual(self.check_controls(client), set())
        defs = coverage.definitions(client)
        self.assertIn('"cad.install"', defs['Cadview.rpc'])
        for action, operation in [('cad-install','start'), ('cad-cancel','cancel'), ('cad-refresh','refresh')]:
            self.assertIn(f'case "{action}": Cadview.rpc(u, value, "{operation}")', defs['Cadview.dispatch.by'])
        hub = (ROOT/'src/core/hub.bend').read_text()
        self.assertIn('"cad.install"', hub)
        self.assertIn('CadInstall{', hub)

    def test_removed_handler_is_detected(self):
        client = (ROOT/'src/core/client.bend').read_text().replace('case "cad-cancel":', 'case "removed-cancel":')
        self.assertEqual(self.check_controls(client), {'cad-cancel'})

    def test_snapshot_is_in_join_and_thin_info(self):
        server = (ROOT/'src/server/server.bend').read_text()
        self.assertIn('J.KV{"cad.install", J.Str{cadInfo}}', coverage.definitions(server)['Hub.info'])
        self.assertIn('Cfg.cad.apply(cfg, tool, snapshot)', coverage.definitions(server)['Hub.cad.progress'])
        self.assertIn('Hub.broadcast', coverage.definitions(server)['Hub.cad.progress'])

if __name__ == '__main__': unittest.main()
