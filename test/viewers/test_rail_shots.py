#!/usr/bin/env python3
"""Regression tests for rail_shots_post.py and rail_shots_compare.py."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_shots_compare as C  # noqa: E402
import rail_shots_post as P  # noqa: E402


def write_non_flat_png(path: Path, color_a=(30, 34, 42), color_b=(220, 120, 40)) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    im = Image.new("RGB", (1440, 900), color_a)
    draw = ImageDraw.Draw(im)
    draw.rectangle([(400, 180), (1100, 720)], fill=color_b)
    im.save(path)


class RailShotsRegressionTests(unittest.TestCase):
    def test_task7_setup_dialog_without_sim_result_is_task_failed(self) -> None:
        """A 'New Simulation Tab' setup dialog with no completed analysis must fail Task 7 (0 pts)."""
        with tempfile.TemporaryDirectory() as td:
            outdir = Path(td)
            full_dir = outdir / "full"
            write_non_flat_png(full_dir / "task7-sim.png", color_b=(90, 140, 200))
            write_non_flat_png(full_dir / "task7-sim-run.png", color_b=(200, 90, 90))

            lay = {
                "topbar": {"x": 0, "y": 0, "w": 1440, "h": 36},
                "main": {"x": 240, "y": 36, "w": 500, "h": 864},
                "rail": {"x": 1368, "y": 36, "w": 72, "h": 864},
            }
            full_parsed = {
                "shots": {
                    "task7-sim": {"state": "ready", "viewer": {}, "layout": lay, "controls_ids": []},
                    "task7-sim-run": {"state": "ready", "viewer": {}, "layout": lay, "controls_ids": []},
                },
                "settled_tags": {},
                "kicad_tags": {
                    "task7-sim": {
                        "state": "ready",
                        "in_gpui_tree": True,
                        "shown": 100,
                        "frames": [{"xid": 100, "kind": "SIMULATOR", "painted": 4000}],
                    }
                },
                "presses": {"press:task7-sim": [{"id": "rail:Simulation", "found": True}]},
            }
            full_caps = {
                "task7-sim": {"name": "task7-sim", "ok": True, "bytes": 1234, "obstructed": False, "obstructions": []},
                "task7-sim-run": {
                    "name": "task7-sim-run",
                    "ok": True,
                    "bytes": 1234,
                    "obstructed": True,
                    "obstructions": [{"xid": "0x1", "title": "New Simulation Tab", "class": ["kicad.real", "KiCad"]}],
                },
            }
            embed_actions = {"sim-run": {"action": "sim-run", "ok": True, "key": "r"}}

            tasks = P.evaluate_scorecard(
                outdir,
                full_parsed,
                full_caps,
                full_client_ok=True,
                embed=True,
                embed_actions=embed_actions,
            )
            t7 = next(t for t in tasks if t["task"] == 7)
            self.assertEqual(t7["outcome"], "task_failed")
            self.assertIn("New Simulation Tab", t7["note"])
            self.assertFalse(t7["evidence"]["sim_result_evidenced"])
            scored = C.score_single_task(t7)
            self.assertTrue(scored["scored"])
            self.assertEqual(scored["points"], 0)

    def test_obstructed_page_is_private_excluded_from_strips_and_unscored(self) -> None:
        """An obstructed shot must be marked private, excluded from headers/rails, and unscored in B3/B6."""
        with tempfile.TemporaryDirectory() as td:
            outdir = Path(td)
            kdir = outdir / "schematic-only"
            write_non_flat_png(kdir / "idle.png")
            write_non_flat_png(kdir / "schematic.png")
            lay = {
                "topbar": {"x": 0, "y": 0, "w": 1440, "h": 36},
                "main": {"x": 240, "y": 36, "w": 500, "h": 864},
                "rail": {"x": 1368, "y": 36, "w": 72, "h": 864},
            }
            parsed = {
                "schematic-only": {
                    "shots": {
                        "idle": {"layout": lay, "viewer": {}},
                        "schematic": {"layout": lay, "viewer": {}},
                    }
                }
            }
            # Only `idle` is cleared; `schematic` is obstructed/private so it must NOT appear in headers.png
            P.build_strips(outdir, ["schematic-only"], parsed, {"schematic-only": {"idle"}})
            self.assertFalse((outdir / "headers.png").exists())
            self.assertTrue((outdir / "rails.png").exists())

            # In consistency scoring, obstructed/private pages are unscored (not penalized or rewarded)
            steps = {
                "kinds": {
                    "schematic-only": {
                        "pages": {
                            "schematic": {
                                "resource_present": True,
                                "rail_state": "enabled",
                                "matched_expected": True,
                                "settled_state": "ready",
                                "obstructed": True,
                                "private": True,
                                "controls_ids": ["ui:Open in KiCad"],
                                "early_capture_ok": True,
                                "viewer": {},
                            }
                        }
                    }
                }
            }
            cons = C.score_consistency(steps)
            self.assertEqual(cons["B3"]["checked"], 0)
            self.assertEqual(cons["B3"]["unscored_obstructed_or_unopened"], 1)
            self.assertEqual(cons["B6"]["loading"]["checked"], 0)
            self.assertEqual(cons["B6"]["unscored_private_or_obstructed"], 1)

    def test_incomparable_manifests_refuse_whole_run_delta_and_keep_unavailable_unscored(self) -> None:
        """Unlike runs (embed vs non-embed) must refuse a whole-run delta and score common tasks only."""
        def make_steps(embed: bool, caps: list[str], t7_outcome: str, t8_outcome: str) -> dict:
            tasks = []
            for tid in range(1, 11):
                if tid in (1, 3, 5, 9, 10):
                    outcome = "passed"
                    pts_parts = {"found": True, "within_budget": True, "result_evidenced": True, "one_step_back": True if tid in (3, 10) else "not_measured"}
                elif tid in (2, 4, 6):
                    outcome = "task_failed"
                    pts_parts = {"found": True, "within_budget": True, "result_evidenced": False, "one_step_back": "not_measured"}
                elif tid == 7:
                    outcome = t7_outcome
                    pts_parts = {"found": False, "within_budget": False, "result_evidenced": False, "one_step_back": "not_measured"}
                else:
                    outcome = t8_outcome
                    pts_parts = {"found": True, "within_budget": True, "result_evidenced": outcome == "passed", "one_step_back": outcome == "passed"}
                tasks.append({
                    "task": tid,
                    "name": f"Task {tid}",
                    "step_budget": 1,
                    "presses_used": 1 if outcome != "unavailable" else 0,
                    "within_budget": True,
                    "outcome": outcome,
                    "parts": pts_parts,
                })
            return {
                "evidence_schema": 2,
                "privacy_checked": True,
                "embed_mode": embed,
                "runtime": {"embed_mode": embed, "capabilities": caps},
                "overall_outcome": "task_failed",
                "kinds": {"full": {"pages": {}, "cleared_shots": []}},
                "scorecard_tasks": tasks,
            }

        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            base_dir = root / "base"
            new_dir = root / "new"
            out_dir = root / "cmp"
            base_dir.mkdir()
            new_dir.mkdir()
            (base_dir / "steps.json").write_text(json.dumps(make_steps(False, [], "unavailable", "unavailable")), encoding="utf-8")
            (new_dir / "steps.json").write_text(json.dumps(make_steps(True, ["sim-frame"], "task_failed", "passed")), encoding="utf-8")

            rc = C.main([str(base_dir), str(new_dir), str(out_dir)])
            self.assertEqual(rc, 0)
            rep = json.loads((out_dir / "score.json").read_text(encoding="utf-8"))
            self.assertFalse(rep["comparable"])
            self.assertIsNone(rep["tasks"]["whole_run_delta"])
            self.assertEqual(rep["tasks"]["common_tasks"]["common_task_ids"], [1, 2, 3, 4, 5, 6, 9, 10])
            self.assertEqual(rep["tasks"]["common_tasks"]["delta"], 0)
            self.assertEqual(rep["tasks"]["base"]["unscored_tasks"], 2)
            self.assertEqual(rep["tasks"]["new"]["unscored_tasks"], 0)

    def test_compare_refuses_non_empty_outdir_and_share_refuses_legacy_manifest(self) -> None:
        """Compare must refuse a reused non-empty OUT_DIR (rc=2) and --share must refuse legacy manifests (rc=3)."""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            base_dir = root / "base"
            new_dir = root / "new"
            out_dir = root / "cmp"
            base_dir.mkdir()
            new_dir.mkdir()
            out_dir.mkdir()
            (out_dir / "stale.txt").write_text("stale\n", encoding="utf-8")

            legacy = {"overall_outcome": "passed", "kinds": {"full": {"pages": {}}}, "scorecard_tasks": []}
            (base_dir / "steps.json").write_text(json.dumps(legacy), encoding="utf-8")
            (new_dir / "steps.json").write_text(json.dumps(legacy), encoding="utf-8")

            # Reused non-empty out_dir refused with rc=2
            self.assertEqual(C.main([str(base_dir), str(new_dir), str(out_dir)]), 2)

            # Fresh out_dir with --share refused with rc=3 because legacy manifests lack privacy_checked
            fresh_out = root / "cmp_share"
            self.assertEqual(C.main(["--share", str(base_dir), str(new_dir), str(fresh_out)]), 3)
            self.assertFalse(fresh_out.exists())

            # Fresh out_dir without --share succeeds, and omits headers-compare/rails-compare
            write_non_flat_png(base_dir / "headers.png")
            write_non_flat_png(new_dir / "headers.png")
            fresh_out2 = root / "cmp_ok"
            self.assertEqual(C.main([str(base_dir), str(new_dir), str(fresh_out2)]), 0)
            self.assertFalse((fresh_out2 / "headers-compare.png").exists())


if __name__ == "__main__":
    unittest.main()
