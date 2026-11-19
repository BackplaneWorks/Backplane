#!/usr/bin/env python3
"""Regression tests for rail-shots.sh, rail_shots_manifest.py, rail_shots_post.py, and rail_shots_compare.py."""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import rail_shots_compare as C  # noqa: E402
import rail_shots_manifest as M  # noqa: E402
import rail_shots_post as P  # noqa: E402


def write_non_flat_png(path: Path, color_a=(30, 34, 42), color_b=(220, 120, 40)) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    im = Image.new("RGB", (1440, 900), color_a)
    draw = ImageDraw.Draw(im)
    draw.rectangle([(400, 180), (1100, 720)], fill=color_b)
    im.save(path)


def make_complete_kind(kind: str, *, embed: bool = False, private: dict[str, list[str]] | None = None, pages: dict | None = None) -> dict:
    priv = dict(private or {})
    expected = M.expected_shots(kind, embed=(embed and kind == "full"))
    cleared = [s for s in expected if s not in priv]
    pgs = dict(pages or {})
    for slug, _, _, _ in M.PAGES:
        if slug not in pgs:
            pgs[slug] = {"settled_state": "ready", "private": slug in priv, "obstructed": False}
    return {"pages": pgs, "cleared_shots": cleared, "private_shots": priv, "missing_shots": []}


def make_schema2_steps(
    *,
    embed: bool = False,
    caps: list[str] | None = None,
    runtime_known: bool = True,
    tasks: list[dict] | None = None,
    kinds: dict | None = None,
) -> dict:
    return {
        "evidence_schema": 2,
        "privacy_checked": True,
        "embed_mode": embed,
        "runtime": ({"embed_mode": embed, "runtime_root": "", "capabilities": caps or []} if runtime_known else None),
        "overall_outcome": "task_failed",
        "kinds": kinds if kinds is not None else {"full": make_complete_kind("full", embed=embed)},
        "scorecard_tasks": tasks if tasks is not None else [],
    }


class RailShotsRegressionTests(unittest.TestCase):
    # --- Item 1: RAIL_KINDS validation before creating any directory ---
    def test_item1_invalid_rail_kinds_rejected_before_creating_outdir(self) -> None:
        for bad in (["../escape"], ["full/sub"], ["unknown"], [""], []):
            with self.assertRaises(ValueError):
                M.validate_kinds(bad)
        self.assertEqual(M.validate_kinds(["full", "board-only"]), ["full", "board-only"])

        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fake_bin = root / "bin"
            fake_bin.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            fake_bin.chmod(0o755)
            assets = root / "assets"
            assets.mkdir()
            outdir = root / "should_not_be_created"
            env = dict(os.environ, RAIL_KINDS="../escape")
            res = subprocess.run(
                [str(HERE / "rail-shots.sh"), str(fake_bin), str(fake_bin), str(assets), str(outdir)],
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertEqual(res.returncode, 2)
            self.assertFalse(outdir.exists(), "OUTDIR must not be created when RAIL_KINDS is invalid")

    # --- Item 2: Fail-closed privacy, --share refuses before steps.json, per-shot clearance, private-* pairs ---
    def test_item2_fail_closed_privacy_and_share_before_steps_json(self) -> None:
        # Missing or failed xwininfo inspection must mark shot private (fail-closed)
        r_fail = P.shot_privacy_reasons({"ok": True, "inspect_ok": False, "inspect_error": "xwininfo died", "obstructed": False}, {})
        self.assertTrue(any("fail-closed" in r for r in r_fail))
        r_missing = P.shot_privacy_reasons({"ok": True, "obstructed": False}, {})
        self.assertTrue(any("fail-closed" in r for r in r_missing))
        r_clean = P.shot_privacy_reasons({"ok": True, "inspect_ok": True, "obstructed": False}, {})
        self.assertEqual(r_clean, [])

        # --share must refuse BEFORE writing steps.json or any derived images
        with tempfile.TemporaryDirectory() as td:
            outdir = Path(td)
            kdir = outdir / "no-design"
            kdir.mkdir(parents=True)
            (kdir / "client.exit").write_text("0\n", encoding="utf-8")
            (kdir / "client.log").write_text("", encoding="utf-8")
            write_non_flat_png(kdir / "idle.png")
            (kdir / "captures.jsonl").write_text(
                json.dumps({"name": "idle", "ok": True, "bytes": 100, "inspect_ok": False, "obstructed": False}) + "\n",
                encoding="utf-8",
            )
            rc = P.main(["--share", str(outdir), "no-design"])
            self.assertEqual(rc, 3)
            self.assertFalse((outdir / "steps.json").exists())
            self.assertFalse((outdir / "headers.png").exists())
            self.assertFalse((outdir / "rails.png").exists())
            self.assertFalse((outdir / "contact-no-design.png").exists())

    def test_item2_compare_per_shot_clearance_and_private_pair_naming(self) -> None:
        # Top-level privacy_checked=True without per-shot cleared_shots must fail has_complete_privacy
        bogus = {
            "evidence_schema": 2,
            "privacy_checked": True,
            "kinds": {"full": {"pages": {"board": {"private": False}}}},
        }
        ok, issues = C.has_complete_privacy(bogus)
        self.assertFalse(ok)
        self.assertTrue(issues)

        # Legacy manifest without positive clearance -> non-share names pair `private-full-board.png`; --share refuses (rc=3)
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            base_dir = root / "base"
            new_dir = root / "new"
            write_non_flat_png(base_dir / "full" / "board.png")
            write_non_flat_png(new_dir / "full" / "board.png")
            legacy = {"overall_outcome": "passed", "kinds": {"full": {"pages": {"board": {"settled_state": "ready"}}}}, "scorecard_tasks": []}
            cleared_new = make_schema2_steps(
                kinds={
                    "full": {
                        "cleared_shots": ["board"],
                        "private_shots": {},
                        "missing_shots": [],
                        "pages": {"board": {"settled_state": "ready", "private": False, "obstructed": False}},
                    }
                }
            )
            (base_dir / "steps.json").write_text(json.dumps(legacy), encoding="utf-8")
            (new_dir / "steps.json").write_text(json.dumps(cleared_new), encoding="utf-8")

            share_out = root / "cmp_share"
            self.assertEqual(C.main(["--share", str(base_dir), str(new_dir), str(share_out)]), 3)
            self.assertFalse(share_out.exists())

            out_dir = root / "cmp_nonshare"
            self.assertEqual(C.main([str(base_dir), str(new_dir), str(out_dir)]), 0)
            self.assertFalse((out_dir / "pairs" / "full-board.png").exists())
            self.assertTrue((out_dir / "pairs" / "private-full-board.png").exists())
            rep = json.loads((out_dir / "score.json").read_text(encoding="utf-8"))
            self.assertEqual(rep["pairs_written"], 0)
            self.assertEqual(rep["pairs_private"], 1)

            # Reused non-empty OUT_DIR refused with rc=2
            self.assertEqual(C.main([str(base_dir), str(new_dir), str(out_dir)]), 2)

    def test_item2_obstructed_page_excluded_from_strips_and_unscored(self) -> None:
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
            parsed = {"schematic-only": {"shots": {"idle": {"layout": lay, "viewer": {}}, "schematic": {"layout": lay, "viewer": {}}}}}
            P.build_strips(outdir, ["schematic-only"], parsed, {"schematic-only": {"idle"}})
            self.assertFalse((outdir / "headers.png").exists())
            self.assertTrue((outdir / "rails.png").exists())

    # --- Item 3: Task 7 setup dialog (task_failed) vs missing observer (unavailable) vs observed pass ---
    def test_item3_task7_observer_and_setup_dialog(self) -> None:
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
                    "task7-sim": {"state": "ready", "in_gpui_tree": True, "shown": 100, "frames": [{"xid": 100, "kind": "SIMULATOR", "painted": 4000}]}
                },
                "presses": {"press:task7-sim": [{"id": "rail:Simulation", "found": True}]},
            }

            # Case A: setup dialog open -> task_failed (0 pts), even with observer=None
            caps_dialog = {
                "task7-sim": {"name": "task7-sim", "ok": True, "bytes": 1234, "inspect_ok": True, "obstructed": False, "obstructions": []},
                "task7-sim-run": {
                    "name": "task7-sim-run", "ok": True, "bytes": 1234, "inspect_ok": True, "obstructed": True,
                    "obstructions": [{"xid": "0x1", "title": "New Simulation Tab", "class": ["kicad.real", "KiCad"]}],
                },
            }
            t7a = next(t for t in P.evaluate_scorecard(outdir, full_parsed, caps_dialog, True, embed=True, embed_actions={"sim-run": {"action": "sim-run", "ok": True, "observer": None}}) if t["task"] == 7)
            self.assertEqual(t7a["outcome"], "task_failed")
            self.assertEqual(C.score_single_task(t7a)["points"], 0)

            # Case B: no setup dialog, but no completion observer -> unavailable (unscored, NEVER passed)
            caps_clean = {
                "task7-sim": {"name": "task7-sim", "ok": True, "bytes": 1234, "inspect_ok": True, "obstructed": False, "obstructions": []},
                "task7-sim-run": {"name": "task7-sim-run", "ok": True, "bytes": 1234, "inspect_ok": True, "obstructed": False, "obstructions": []},
            }
            t7b = next(t for t in P.evaluate_scorecard(outdir, full_parsed, caps_clean, True, embed=True, embed_actions={"sim-run": {"action": "sim-run", "ok": True, "observer": None}}) if t["task"] == 7)
            self.assertEqual(t7b["outcome"], "unavailable")
            self.assertFalse(C.score_single_task(t7b)["scored"])

            # Case C: completion observer reports evidenced result -> passed
            t7c = next(t for t in P.evaluate_scorecard(outdir, full_parsed, caps_clean, True, embed=True, embed_actions={"sim-run": {"action": "sim-run", "ok": True, "observer": "spice_result", "result_evidenced": True, "result_summary": "V(out)=1.25V"}}) if t["task"] == 7)
            self.assertEqual(t7c["outcome"], "passed")
            self.assertEqual(C.score_single_task(t7c)["points"], 3)  # one_step_back not_measured

    # --- Item 4: Unknown runtime, identical scored task IDs for whole-run delta, common-task join on id+name+budget ---
    def test_item4_unknown_runtime_scored_id_sets_and_common_task_join(self) -> None:
        # Two unknown runtimes are NOT comparable
        u1 = make_schema2_steps(runtime_known=False)
        u2 = make_schema2_steps(runtime_known=False)
        comp, reasons = C.check_comparability(u1, u2)
        self.assertFalse(comp)
        self.assertTrue(any("unknown" in r for r in reasons))

        # Different scored task ID sets (even same count!) -> whole_run_delta is None;
        # common-task join excludes a task whose budget/definition differs.
        def mk_task(tid: int, name: str, budget: int, outcome: str) -> dict:
            return {
                "task": tid, "name": name, "step_budget": budget, "presses_used": 1 if outcome != "unavailable" else 0,
                "within_budget": True, "outcome": outcome,
                "parts": {"found": True, "within_budget": True, "result_evidenced": outcome == "passed", "one_step_back": True},
            }

        base_tasks = [
            mk_task(1, "Open the board", 1, "passed"),
            mk_task(2, "Open the 3D view", 1, "passed"),
            mk_task(7, "Sim", 2, "unavailable"),
            mk_task(8, "Edit", 1, "passed"),
        ]
        new_tasks = [
            mk_task(1, "Open the board", 1, "passed"),
            mk_task(2, "Open the 3D view", 2, "passed"),  # budget changed -> excluded from common tasks!
            mk_task(7, "Sim", 2, "passed"),               # scored here, unavailable in base -> scored id sets differ!
            mk_task(8, "Edit", 1, "unavailable"),
        ]
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            bdir, ndir, odir = root / "b", root / "n", root / "o"
            bdir.mkdir()
            ndir.mkdir()
            (bdir / "steps.json").write_text(json.dumps(make_schema2_steps(caps=[], tasks=base_tasks)), encoding="utf-8")
            (ndir / "steps.json").write_text(json.dumps(make_schema2_steps(caps=[], tasks=new_tasks)), encoding="utf-8")
            self.assertEqual(C.main([str(bdir), str(ndir), str(odir)]), 0)
            rep = json.loads((odir / "score.json").read_text(encoding="utf-8"))
            self.assertFalse(rep["comparable"])
            self.assertIsNone(rep["tasks"]["whole_run_delta"])
            self.assertEqual(rep["tasks"]["common_tasks"]["common_task_ids"], [1])
            self.assertEqual(rep["tasks"]["common_tasks"]["excluded_definition_mismatch"], [2])

    # --- Item 5: Exact #1317 rules, B6 loading/empty honesty, B4 partial, independent 4-part task records ---
    def test_item5_scores_rules_b6_loading_empty_b4_partial_and_task_parts(self) -> None:
        # Exact 10 rules from issue #1317 (verify B9 is embedded KiCad tab strip agreeing with rail)
        self.assertEqual(len(C.SCORECARD_RULES), 10)
        self.assertIn("embedded KiCad tab strip", C.RULE_BY_ID["B9"])

        # B6 loading: generic early capture without state=="loading" + text is UNSCORED (not a pass);
        # empty without words fails; B4 has points=None (partial check).
        steps = make_schema2_steps(
            kinds={
                "no-design": {
                    "cleared_shots": ["board", "files", "review"],
                    "private_shots": {},
                    "missing_shots": [],
                    "pages": {
                        "board": {
                            "resource_present": False, "rail_state": "disabled", "settled_state": "disabled",
                            "matched_expected": False, "obstructed": False, "private": False,
                        },
                        "files": {
                            "resource_present": True, "rail_state": "enabled", "settled_state": "ready",
                            "matched_expected": True, "obstructed": False, "private": False,
                            "early_capture_ok": True, "early_300ms_state": "ready", "early_loading_text": "",
                            "controls_ids": ["rail:Files"], "viewer": {},
                        },
                        "review": {
                            "resource_present": True, "rail_state": "enabled", "settled_state": "empty",
                            "empty_text": "",  # missing empty-state words -> fails empty check
                            "matched_expected": True, "obstructed": False, "private": False,
                            "early_capture_ok": True, "early_300ms_state": "loading", "early_loading_text": "Loading…",
                            "controls_ids": ["rail:Review"], "viewer": {},
                        },
                    },
                }
            }
        )
        cons = C.score_consistency(steps)
        self.assertIsNone(cons["B4"]["points"])  # partial check, no 3/3
        self.assertEqual(cons["B4"]["clean"], 1)
        # `files` generic early capture was unscored; `review` had state=="loading" + text -> 1/1
        self.assertEqual(cons["B6"]["loading"]["checked"], 1)
        self.assertEqual(cons["B6"]["loading"]["clean"], 1)
        self.assertEqual(cons["B6"]["loading"]["unscored_non_loading"], 1)
        # `review` empty state with empty_text=="" -> 0/1
        self.assertEqual(cons["B6"]["empty"]["checked"], 1)
        self.assertEqual(cons["B6"]["empty"]["clean"], 0)
        self.assertEqual(cons["B6"]["empty"]["points"], 0.0)

        # Failed task keeps the parts it earned in `parts`, while total points == 0
        failed_task = {
            "task": 2, "name": "Open the 3D view", "step_budget": 1, "presses_used": 1, "within_budget": True,
            "outcome": "task_failed",
            "parts": {"found": True, "within_budget": True, "result_evidenced": False, "one_step_back": "not_measured"},
        }
        st = C.score_single_task(failed_task)
        self.assertEqual(st["points"], 0)
        self.assertTrue(st["parts"]["found"])
        self.assertTrue(st["parts"]["within_budget"])
        self.assertFalse(st["parts"]["result_evidenced"])
        self.assertEqual(st["unmeasured_parts"], ["one_step_back"])

    # --- Final re-review item 1: manifest-wide per-shot clearance (early + task shots) & valid manifests for shareable pairs ---
    def test_final1_privacy_validates_early_and_task_shots_and_gates_shareable_pairs(self) -> None:
        # Removing the 3 private embed task shots from private_shots (so they are in neither cleared nor private nor missing)
        # must fail has_complete_privacy, fail --share, and prevent any pair from getting a shareable name.
        tampered = make_schema2_steps(embed=True, caps=["sim-frame"])
        tampered["kinds"]["full"]["cleared_shots"] = [
            s for s in tampered["kinds"]["full"]["cleared_shots"] if s not in ("task8-edit", "task7-sim", "task7-sim-run")
        ]
        ok, issues = C.has_complete_privacy(tampered)
        self.assertFalse(ok)
        self.assertTrue(any("task7-sim" in i for i in issues))

        # A shot appearing in BOTH cleared_shots and private_shots must also be rejected
        dup = make_schema2_steps(embed=False)
        dup["kinds"]["full"]["private_shots"]["board-300ms"] = ["obstructed"]
        ok_dup, issues_dup = C.has_complete_privacy(dup)
        self.assertFalse(ok_dup)
        self.assertTrue(any("multiple clearance sets" in i for i in issues_dup))

        # Even if `board` itself is in cleared_shots on both sides, a tampered manifest prevents shareable pair names
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            bdir, ndir = root / "b", root / "n"
            write_non_flat_png(bdir / "full" / "board.png")
            write_non_flat_png(ndir / "full" / "board.png")
            valid = make_schema2_steps(embed=True, caps=["sim-frame"])
            (bdir / "steps.json").write_text(json.dumps(tampered), encoding="utf-8")
            (ndir / "steps.json").write_text(json.dumps(valid), encoding="utf-8")

            share_out = root / "cmp_share"
            self.assertEqual(C.main(["--share", str(bdir), str(ndir), str(share_out)]), 3)
            self.assertFalse(share_out.exists())

            out_dir = root / "cmp_nonshare"
            self.assertEqual(C.main([str(bdir), str(ndir), str(out_dir)]), 0)
            self.assertFalse((out_dir / "pairs" / "full-board.png").exists())
            self.assertTrue((out_dir / "pairs" / "private-full-board.png").exists())

    # --- Final re-review item 2: Task 7 requires non-empty identifiable result_summary ---
    def test_final2_task7_rejects_empty_result_summary(self) -> None:
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
                    "task7-sim": {"state": "ready", "in_gpui_tree": True, "shown": 100, "frames": [{"xid": 100, "kind": "SIMULATOR", "painted": 4000}]}
                },
                "presses": {"press:task7-sim": [{"id": "rail:Simulation", "found": True}]},
            }
            caps_clean = {
                "task7-sim": {"name": "task7-sim", "ok": True, "bytes": 1234, "inspect_ok": True, "obstructed": False, "obstructions": []},
                "task7-sim-run": {"name": "task7-sim-run", "ok": True, "bytes": 1234, "inspect_ok": True, "obstructed": False, "obstructions": []},
            }
            t7_empty = next(
                t for t in P.evaluate_scorecard(
                    outdir, full_parsed, caps_clean, True, embed=True,
                    embed_actions={"sim-run": {"action": "sim-run", "ok": True, "observer": "spice_result", "result_evidenced": True, "result_summary": "   "}},
                )
                if t["task"] == 7
            )
            self.assertEqual(t7_empty["outcome"], "task_failed")
            self.assertFalse(t7_empty["evidence"]["sim_result_evidenced"])
            self.assertEqual(C.score_single_task(t7_empty)["points"], 0)

    # --- Final re-review item 3: click metric delivery (`changed: true` / `action`) sets `found: true` ---
    def test_final3_click_metric_sets_found_true_for_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            log = Path(td) / "client.log"
            log.write_text(
                "\n".join([
                    'GPUI_METRIC {"event":"mark","name":"press:task7-sim"}',
                    'GPUI_METRIC {"id":"rail:Simulation","changed":true,"action":"kicad simulation","event":"click"}',
                    'GPUI_METRIC {"event":"mark","name":"press:task8-edit"}',
                    'GPUI_METRIC {"id":"ui:Edit","changed":true,"action":"kicad edit true","event":"click"}',
                    'GPUI_METRIC {"event":"mark","name":"press:missing"}',
                    'GPUI_METRIC {"id":"ui:Nope","error":"not found","event":"click"}',
                ]) + "\n",
                encoding="utf-8",
            )
            parsed = P.parse_client_log(log)
            self.assertTrue(parsed["presses"]["press:task7-sim"][0]["found"])
            self.assertTrue(parsed["presses"]["press:task8-edit"][0]["found"])
            self.assertFalse(parsed["presses"]["press:missing"][0]["found"])


if __name__ == "__main__":
    unittest.main()
