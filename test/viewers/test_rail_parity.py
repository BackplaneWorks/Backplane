#!/usr/bin/env python3
"""rail_parity.py: per-check parity between two runs' steps.json."""

from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_parity as R  # noqa: E402

BASE = {
    "scorecard_tasks": [{"task": 1, "outcome": "passed", "parts": {"found": True, "one_step_back": True}}, {"task": 4, "outcome": "task_failed", "parts": {"found": True}}],
    "kinds": {"full": {"b1_checks": {"entries": {"board": {"status": "measured", "passed": True, "checks": {"press_open": True, "press_close": True, "key_open": True}},
                                                 "split": {"status": "unavailable", "passed": None, "checks": {"disabled_press": "unavailable"}}}}}},
    "extra_checks": [{"id": "full/escape-after-closed-probe", "outcome": "passed"}, {"id": "board-only/missing-schematic", "outcome": "passed"}],
}


class ParityTests(unittest.TestCase):
    def test_identical_runs_have_zero_differences_and_new_gates_are_listed_not_counted(self) -> None:
        new = copy.deepcopy(BASE)
        new["extra_checks"].append({"id": "full/auto-a-frame-no-keyboard", "outcome": "passed"})
        res = R.compare(BASE, new)
        self.assertEqual(res["differences"], [])
        self.assertEqual(res["total"], 6 + 2 + 3 + 1 + 2 + 1 + 1 + 1 - 1)  # tasks(3 per task) + b1 (status,passed,+checks per entry) + 2 extra
        text = R.render(res)
        self.assertTrue(text.startswith(f"0 differences of {res['total']} checks"))
        self.assertIn("full/auto-a-frame-no-keyboard", text)

    def test_every_kind_of_difference_is_found(self) -> None:
        def diff(mut):
            new = copy.deepcopy(BASE)
            mut(new)
            return [d[0] for d in R.compare(BASE, new)["differences"]]
        self.assertEqual(diff(lambda n: n["scorecard_tasks"][0].update(outcome="task_failed")), ["task 1: outcome"])
        self.assertEqual(diff(lambda n: n["kinds"]["full"]["b1_checks"]["entries"]["board"]["checks"].update(press_close=False)), ["B1 full/board: press_close"])
        self.assertEqual(diff(lambda n: n["kinds"]["full"]["b1_checks"]["entries"]["split"]["checks"].update(disabled_press=True))[0], "B1 full/split: disabled_press")
        self.assertEqual(diff(lambda n: n["extra_checks"][1].update(outcome="failed")), ["extra board-only/missing-schematic"])
        # a check that only one run has (a transition, an entry, a task, an extra check) is a difference too
        self.assertEqual(diff(lambda n: n["kinds"]["full"]["b1_checks"]["entries"]["board"]["checks"].pop("key_open")), ["B1 full/board: key_open"])
        self.assertIn("task 9: outcome", diff(lambda n: n["scorecard_tasks"].append({"task": 9, "outcome": "passed", "parts": {}})))
        self.assertEqual(diff(lambda n: n["extra_checks"].pop()), ["extra board-only/missing-schematic"])
        self.assertEqual(diff(lambda n: n["extra_checks"].append({"id": "full/new-ungated", "outcome": "passed"})), ["extra full/new-ungated"])

    def test_cli_exit_status_and_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            a, b, md = Path(td) / "a.json", Path(td) / "b.json", Path(td) / "t.md"
            a.write_text(json.dumps(BASE), encoding="utf-8")
            b.write_text(json.dumps(BASE), encoding="utf-8")
            self.assertEqual(R.main(["rail_parity.py", str(a), str(b), "--md", str(md)]), 0)
            self.assertIn("0 differences of", md.read_text(encoding="utf-8"))
            changed = copy.deepcopy(BASE)
            changed["scorecard_tasks"][1]["outcome"] = "passed"
            b.write_text(json.dumps(changed), encoding="utf-8")
            self.assertEqual(R.main(["rail_parity.py", str(a), str(b), "--md", str(md)]), 1)
            self.assertIn("| task 4: outcome | task_failed | passed |", md.read_text(encoding="utf-8"))
            self.assertEqual(R.main(["rail_parity.py", str(a)]), 2)


if __name__ == "__main__":
    unittest.main()
