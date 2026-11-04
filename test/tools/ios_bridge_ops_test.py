#!/usr/bin/env python3
"""The iOS app answers every op the phone apps' bridge shim posts.

mobile/shell/backplane-app.js (shared by both apps) posts {op} with ask("op")
or tell("op"); mobile/ios/Backplane/WebShell.swift handles each in a
`case "op":`. Nothing builds Swift on Linux, so a new op the iOS side misses
would only show on a phone: this checks the two lists agree.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SHIM = (ROOT / 'mobile/shell/backplane-app.js').read_text(encoding='utf-8')
SWIFT = (ROOT / 'mobile/ios/Backplane/WebShell.swift').read_text(encoding='utf-8')


class IosBridgeOps(unittest.TestCase):
    def test_every_shim_op_has_a_case(self):
        ops = set(re.findall(r'\b(?:ask|tell)\("([A-Za-z]+)"', SHIM))
        self.assertIn('hello', ops)
        cases = set(re.findall(r'case "([A-Za-z]+)"', SWIFT))
        self.assertEqual(sorted(ops - cases), [], 'ops the iOS shell does not answer')

    def test_handler_and_placeholder(self):
        self.assertIn('window.webkit.messageHandlers.backplane', SHIM)
        self.assertIn('static let channel = "backplane"', SWIFT)
        self.assertEqual(SHIM.count('/*@info*/null'), 1)
        self.assertIn('"/*@info*/null"', SWIFT)

    def test_info_keys(self):
        keys = set(re.findall(r'info\.([a-zA-Z]+)', SHIM))
        block = SWIFT[SWIFT.index('let info: [String: Any]'):]
        block = block[:block.index(']\n')]
        given = set(re.findall(r'"([a-zA-Z]+)":', block))
        self.assertEqual(sorted(keys - given), [], 'facts the shim reads that iOS does not give')


if __name__ == '__main__':
    unittest.main()
