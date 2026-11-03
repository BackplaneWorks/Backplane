"""Focused #790/#794 literal handler inventory; runtime effects are tested by
project_home_e2e and the real-renderer --home-parity acceptance gate.
"""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('home_coverage', ROOT / 'scripts/ahead-validation/coverage.py')
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


def home_actions(source):
    result = coverage.inventory(source)
    actions = [a for a in result['actions'] if a['file'] == 'src/web/project-home.bend']
    computed = [a for a in result['computed_actions'] if a['file'] == 'src/web/project-home.bend']
    return actions, computed


class HandlerCoverage(unittest.TestCase):
    def test_every_home_control_has_a_reachable_handler(self):
        actions, computed = home_actions(coverage.Source(root=ROOT))
        self.assertFalse(computed, 'computed actions require explicit coverage')
        self.assertFalse([a for a in actions if not a['covered']])
        self.assertTrue({'home-viewer-size', 'home-view', 'home-bom', 'home-model', 'panel'} <= {a['action'] for a in actions})

    def test_deleted_expansion_handler_is_detected(self):
        class MissingHandler(coverage.Source):
            def read(self, path):
                text = super().read(path)
                if path == 'src/core/client.bend':
                    text = text.replace('case "home-viewer-size": Home.viewer.size(u, value)', 'case "removed-size-handler": Home.viewer.size(u, value)')
                return text
        actions, _ = home_actions(MissingHandler(root=ROOT))
        self.assertTrue(any(a['action'] == 'home-viewer-size' and not a['covered'] for a in actions))


if __name__ == '__main__':
    unittest.main()
