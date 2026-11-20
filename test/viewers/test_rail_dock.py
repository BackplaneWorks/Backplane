#!/usr/bin/env python3
"""Unit tests for docking/tiling Task 4 and Rule B7 measurement (`rail_dock.py`)."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_dock as DOCK  # noqa: E402
import rail_shots_compare as C  # noqa: E402
import rail_shots_post as P  # noqa: E402


def _write_painted_png(path: Path, tiles: list[tuple[dict, tuple[int, int, int], bool]], size=(1440, 900)) -> None:
    """Write a synthetic screenshot with optional patterned (non-flat) or solid (flat) tile bodies."""
    im = Image.new("RGB", size, (13, 15, 18))
    draw = ImageDraw.Draw(im)
    for rect, color, patterned in tiles:
        x, y, w, h = int(rect["x"]), int(rect["y"]), int(rect["w"]), int(rect["h"])
        draw.rectangle([x, y, x + w - 1, y + h - 1], fill=(20, 22, 26))
        if patterned:
            for k in range(0, min(w, h) - 40, 14):
                draw.rectangle([x + 12 + k, y + 44 + k // 2, x + 28 + k, y + 60 + k // 2], fill=color)
    im.save(path)


def _ok_cap(obs=False, insp=True) -> dict:
    return {"ok": True, "inspect_ok": insp, "obstructed": obs, "obstructions": [{"title": "Dlg"}] if obs else []}


class RailDockHarnessTests(unittest.TestCase):
    def test_unavailable_without_tiles_metric(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            parsed = {"tiles_metric_seen": False, "shots": {}}
            self.assertIsNone(DOCK.evaluate_task4_dock(fdir, parsed, {}, True))
            b7 = DOCK.evaluate_b7_dock(fdir, parsed, {})
            self.assertEqual(b7["status"], "unavailable")
            self.assertEqual(b7["checked"], 0)
            for k in DOCK.DOCK_B7_CHECKS:
                self.assertEqual(b7["checks"][k]["status"], "unavailable")

            # Compare reports B7 as unavailable (never a false pass or fail)
            cons = C.score_consistency({"kinds": {"full": {"b7_checks": b7}}})
            self.assertEqual(cons["B7"]["status"], "unavailable")
            self.assertIsNone(cons["B7"]["points"])

    def test_parse_client_log_extracts_tiles_drag_and_refusals(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            log = Path(td) / "client.log"
            log.write_text("\n".join([
                'GPUI_METRIC {"event":"mark","name":"b7:resize-drag"}',
                'GPUI_METRIC {"event":"tiles","tiles":[{"id":"t0","kind":"board","rect":{"x":753,"y":36,"w":300,"h":800},"focused":true},{"id":"t1","kind":"schematic","rect":{"x":1057,"y":36,"w":310,"h":800},"focused":false}],"splitters":[{"id":"sp0","rect":{"x":1053,"y":36,"w":4,"h":800},"axis":"x"}]}',
                'GPUI_METRIC {"event":"drag","ok":true,"from":[1055,400],"to":[980,400],"moves":4}',
                'GPUI_METRIC {"event":"shot-ready","name":"b7-after-resize","state":"ready","settled":true,"viewer":{"open":"board"}}',
                'GPUI_METRIC {"event":"mark","name":"b7:overlay-incompatible"}',
                'GPUI_LOG {"refused":"Those views cannot be overlaid"}',
            ]) + "\n", encoding="utf-8")
            parsed = P.parse_client_log(log)
            self.assertTrue(parsed["tiles_metric_seen"])
            self.assertEqual(len(parsed["shots"]["b7-after-resize"]["tiles"]), 2)
            self.assertEqual(len(parsed["shots"]["b7-after-resize"]["splitters"]), 1)
            self.assertTrue(parsed["drags"]["b7:resize-drag"][0]["ok"])
            self.assertEqual(parsed["refusals"]["b7:overlay-incompatible"][0]["text"], "Those views cannot be overlaid")

    def test_task4_dock_passes_fails_and_enforces_privacy(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            lay = {"topbar": {"x": 0, "y": 0, "w": 1440, "h": 36}, "main": {"x": 264, "y": 36, "w": 488, "h": 835}, "rail": {"x": 1369, "y": 36, "w": 71, "h": 835}}
            r_full = {"x": 753, "y": 36, "w": 616, "h": 835}
            r_left = {"x": 753, "y": 36, "w": 306, "h": 835}
            r_right = {"x": 1063, "y": 36, "w": 306, "h": 835}
            sp_x = {"id": "sp0", "rect": {"x": 1059, "y": 36, "w": 4, "h": 835}, "axis": "x"}

            _write_painted_png(fdir / "task4-dock-gerbers.png", [(r_full, (60, 180, 120), True)])
            _write_painted_png(fdir / "task4-dock-split.png", [(r_left, (60, 180, 120), True), (r_right, (200, 160, 60), True)])
            _write_painted_png(fdir / "task4-dock-back.png", [(r_full, (60, 180, 120), True)])

            parsed = {
                "tiles_metric_seen": True,
                "shots": {
                    "task4-dock-gerbers": {
                        "state": "ready", "layout": lay, "tiles": [{"id": "t0", "kind": "gerbers", "rect": r_full, "focused": True}], "splitters": [], "viewer": {},
                    },
                    "task4-dock-split": {
                        "state": "ready", "layout": lay,
                        "tiles": [
                            {"id": "t0", "kind": "gerbers", "rect": r_left, "focused": False},
                            {"id": "t1", "kind": "board", "rect": r_right, "focused": True},
                        ],
                        "splitters": [sp_x],
                        "viewer": {"tiles": {"t0": {"source": "mfg/gerbers"}, "t1": {"source": "ecc83-pp.kicad_pcb"}}},
                    },
                    "task4-dock-back": {
                        "state": "ready", "layout": lay, "tiles": [{"id": "t0", "kind": "gerbers", "rect": r_full, "focused": True}], "splitters": [], "viewer": {},
                    },
                },
            }
            caps = {"task4-dock-gerbers": _ok_cap(), "task4-dock-split": _ok_cap(), "task4-dock-back": _ok_cap()}

            # 1. Clean pass
            res = DOCK.evaluate_task4_dock(fdir, parsed, caps, True)
            self.assertEqual(res["outcome"], "passed", res)
            self.assertTrue(res["one_step_back"])

            # 2. Missing splitter or flat body -> task_failed
            parsed_nosp = json.loads(json.dumps(parsed))
            parsed_nosp["shots"]["task4-dock-split"]["splitters"] = []
            res_nosp = DOCK.evaluate_task4_dock(fdir, parsed_nosp, caps, True)
            self.assertEqual(res_nosp["outcome"], "task_failed")
            self.assertIn("splitter", res_nosp["note"])

            # 3. Same source on both tiles -> task_failed
            parsed_same = json.loads(json.dumps(parsed))
            parsed_same["shots"]["task4-dock-split"]["viewer"]["tiles"]["t1"]["source"] = "mfg/gerbers"
            self.assertEqual(DOCK.evaluate_task4_dock(fdir, parsed_same, caps, True)["outcome"], "task_failed")

            # 4. Uncleared / obstructed shot -> harness_failed (fail-closed)
            caps_obs = {**caps, "task4-dock-split": _ok_cap(obs=True)}
            self.assertEqual(DOCK.evaluate_task4_dock(fdir, parsed, caps_obs, True)["outcome"], "harness_failed")

    def test_b7_all_seven_checks_pass_and_detect_faults(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            lay = {"topbar": {"x": 0, "y": 0, "w": 1440, "h": 36}, "main": {"x": 264, "y": 36, "w": 488, "h": 835}, "rail": {"x": 1369, "y": 36, "w": 71, "h": 835}}
            r_full = {"x": 753, "y": 36, "w": 616, "h": 835}
            r_l = {"x": 753, "y": 36, "w": 306, "h": 835}
            r_r = {"x": 1063, "y": 36, "w": 306, "h": 835}
            r_t = {"x": 753, "y": 36, "w": 616, "h": 415}
            r_b = {"x": 753, "y": 455, "w": 616, "h": 416}
            r_l_narrow = {"x": 753, "y": 36, "w": 220, "h": 835}
            r_r_wide = {"x": 977, "y": 36, "w": 392, "h": 835}
            sp_x = {"id": "sp0", "rect": {"x": 1059, "y": 36, "w": 4, "h": 835}, "axis": "x"}
            sp_x_moved = {"id": "sp0", "rect": {"x": 973, "y": 36, "w": 4, "h": 835}, "axis": "x"}
            sp_y = {"id": "sp0", "rect": {"x": 753, "y": 451, "w": 616, "h": 4}, "axis": "y"}

            # Generate cleared synthetic PNGs for every B7 shot
            c_brd = (200, 160, 60)
            c_sch = (80, 160, 230)
            c_ov = (230, 80, 190)
            _write_painted_png(fdir / "b7-add-right.png", [(r_l, c_brd, True), (r_r, c_sch, True)])
            _write_painted_png(fdir / "b7-add-left.png", [(r_l, c_sch, True), (r_r, c_brd, True)])
            _write_painted_png(fdir / "b7-add-bottom.png", [(r_t, c_brd, True), (r_b, c_sch, True)])
            _write_painted_png(fdir / "b7-add-top.png", [(r_t, c_sch, True), (r_b, c_brd, True)])
            _write_painted_png(fdir / "b7-close-right.png", [(r_full, c_brd, True)])
            _write_painted_png(fdir / "b7-before-swap.png", [(r_l, c_brd, True), (r_r, c_sch, True)])
            _write_painted_png(fdir / "b7-after-swap.png", [(r_l, c_sch, True), (r_r, c_brd, True)])
            _write_painted_png(fdir / "b7-after-resize.png", [(r_l_narrow, (120, 220, 140), True), (r_r_wide, (220, 110, 90), True)])
            _write_painted_png(fdir / "b7-maximized.png", [(r_full, c_sch, True)])
            _write_painted_png(fdir / "b7-restored.png", [(r_l_narrow, c_sch, True), (r_r_wide, c_brd, True)])
            _write_painted_png(fdir / "b7-overlay-plain.png", [(r_full, c_brd, True)])
            _write_painted_png(fdir / "b7-overlay-on.png", [(r_full, c_ov, True)])
            _write_painted_png(fdir / "b7-overlay-off.png", [(r_full, c_brd, True)])
            _write_painted_png(fdir / "b7-overlay-incompat.png", [(r_l, c_brd, True), (r_r, c_sch, True)])

            shots = {
                "b7-add-right": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_l}, {"id": "t1", "kind": "schematic", "rect": r_r}], "splitters": [sp_x], "viewer": {}},
                "b7-add-left": {"layout": lay, "tiles": [{"id": "t1", "kind": "schematic", "rect": r_l}, {"id": "t0", "kind": "board", "rect": r_r}], "splitters": [sp_x], "viewer": {}},
                "b7-add-bottom": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_t}, {"id": "t1", "kind": "schematic", "rect": r_b}], "splitters": [sp_y], "viewer": {}},
                "b7-add-top": {"layout": lay, "tiles": [{"id": "t1", "kind": "schematic", "rect": r_t}, {"id": "t0", "kind": "board", "rect": r_b}], "splitters": [sp_y], "viewer": {}},
                "b7-close-right": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_full}], "splitters": [], "viewer": {}},
                "b7-before-swap": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_l}, {"id": "t1", "kind": "schematic", "rect": r_r}], "splitters": [sp_x], "viewer": {}},
                "b7-after-swap": {"layout": lay, "tiles": [{"id": "t1", "kind": "schematic", "rect": r_l}, {"id": "t0", "kind": "board", "rect": r_r}], "splitters": [sp_x], "viewer": {}},
                "b7-after-resize": {"layout": lay, "tiles": [{"id": "t1", "kind": "schematic", "rect": r_l_narrow}, {"id": "t0", "kind": "board", "rect": r_r_wide}], "splitters": [sp_x_moved], "viewer": {}},
                "b7-maximized": {"layout": lay, "tiles": [{"id": "t1", "kind": "schematic", "rect": r_full}], "splitters": [], "viewer": {}},
                "b7-restored": {"layout": lay, "tiles": [{"id": "t1", "kind": "schematic", "rect": r_l_narrow}, {"id": "t0", "kind": "board", "rect": r_r_wide}], "splitters": [sp_x_moved], "viewer": {}},
                "b7-overlay-plain": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_full}], "splitters": [], "viewer": {}},
                "b7-overlay-on": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_full, "overlay": {"kind": "gerbers"}}], "splitters": [], "viewer": {"tiles": {"t0": {"overlay": {"kind": "gerbers"}}}}},
                "b7-overlay-off": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_full}], "splitters": [], "viewer": {"tiles": {"t0": {}}}},
                "b7-overlay-incompat": {"layout": lay, "tiles": [{"id": "t0", "kind": "board", "rect": r_l}, {"id": "t1", "kind": "files", "rect": r_r}], "splitters": [sp_x], "viewer": {}},
            }
            caps = {k: _ok_cap() for k in shots}
            parsed = {
                "tiles_metric_seen": True,
                "shots": shots,
                "drags": {"b7:resize-drag": [{"ok": True, "from": [1061, 420], "to": [975, 420], "moves": 6}]},
                "refusals": {"b7:overlay-incompatible": [{"text": "Those views cannot be overlaid"}]},
            }

            b7 = DOCK.evaluate_b7_dock(fdir, parsed, caps)
            self.assertEqual(b7["status"], "measured", b7)
            self.assertEqual(b7["checked"], 7)
            self.assertEqual(b7["clean"], 7)
            self.assertEqual(b7["violations"], [])

            cons = C.score_consistency({"kinds": {"full": {"b7_checks": b7}}})
            self.assertEqual(cons["B7"]["status"], "measured")
            self.assertEqual(cons["B7"]["points"], 3.0)

            # Raw error code in refusal -> incompatible_overlay_refusal fails
            parsed_raw = {**parsed, "refusals": {"b7:overlay-incompatible": [{"text": "Refusal::IncompatibleOverlay"}]}}
            b7_raw = DOCK.evaluate_b7_dock(fdir, parsed_raw, caps)
            self.assertFalse(b7_raw["checks"]["incompatible_overlay_refusal"]["passed"])

            # Missing drag event -> resize_drag is unavailable (never a false pass)
            parsed_nodrag = {**parsed, "drags": {}}
            b7_nd = DOCK.evaluate_b7_dock(fdir, parsed_nodrag, caps)
            self.assertEqual(b7_nd["checks"]["resize_drag"]["status"], "unavailable")
            self.assertEqual(b7_nd["status"], "measured_partial")


if __name__ == "__main__":
    unittest.main()
