"""Tie the secret-action law to rendered controls and actual dispatcher cases."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]

class SecretBoxHandlers(unittest.TestCase):
    def test_every_rendered_action_has_an_owner(self):
        client = (ROOT/'src/core/client.bend').read_text()
        policy = (ROOT/'src/core/secret-box.bend').read_text()
        actions = re.search(r'def Sbox.actions\(\).*?\n(.*?)(?=\ndef )', policy, re.S)[1]
        names = re.findall(r'"(secret-[\w-]+)"', actions)
        dispatch = re.search(r'def Sbox.act.k\(.*?\n(.*?)(?=\ndef )', client, re.S)[1]
        cases = set(map(int, re.findall(r'case (\d+)n:', dispatch)))
        self.assertEqual(cases, set(range(len(names)-1)))
        self.assertIn('Sbox.act.k(SBOX.Sbox.route(action), u, value)', client)
        controls = ''
        for file, start, end in [('src/web/view.bend','def View.sbox.k','def View.secrets.rows'), ('src/mobile/screen.bend','def Mob.sbox.k','def Mob.secrets.rows')]:
            text = (ROOT/file).read_text()
            controls += text[text.index(start):text.index(end)]
        rendered = set(re.findall(r'"(secret-[\w-]+)"', controls))
        self.assertFalse(rendered - set(names) - {'secret-save'})
        host = (ROOT/'src/web/host.js').read_text()
        self.assertIn('if (action === "secret-save") return secretSave(value);', host)
        self.assertIn('secretSaves.receive(envelope)', host)
        self.assertIn('thin?.client.links.get(location.origin)?.accountBinding', host)
        self.assertIn('secretSaves.paint()', host)
        mobile = (ROOT/'src/mobile/bridge.js').read_text()
        self.assertIn('secretSaves.receive(k, plain(o))', mobile)
        self.assertNotIn('this.act("secret-sent", "")', mobile)

if __name__ == '__main__':
    unittest.main()
