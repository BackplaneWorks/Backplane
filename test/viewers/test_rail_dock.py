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
import rail_shots_driver as D  # noqa: E402
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


def _body_of(rect: dict) -> dict:
    return {"x": rect["x"], "y": rect["y"] + 32, "w": rect["w"], "h": rect["h"] - 32}


def _ok_cap(obs=False, insp=True) -> dict:
    return {"ok": True, "inspect_ok": insp, "obstructed": obs, "obstructions": [{"title": "Dlg"}] if obs else []}


class RailDockHarnessTests(unittest.TestCase):
    def test_unavailable_without_tiles_metric_and_b7_scored_over_all_seven(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            parsed = {"tiles_metric_seen": False, "shots": {}}
            self.assertIsNone(DOCK.evaluate_task4_dock(fdir, parsed, {}, True))
            b7 = DOCK.evaluate_b7_dock(fdir, parsed, {})
            self.assertEqual(b7["status"], "unavailable")
            self.assertEqual(b7["checked"], 0)
            self.assertEqual(b7["total"], 7)

            cons = C.score_consistency({"kinds": {"full": {"b7_checks": b7}}})
            self.assertEqual(cons["B7"]["status"], "unavailable")
            self.assertIsNone(cons["B7"]["points"])

            # 1 measured pass + 6 unavailable must NOT earn 3/3 (must be 1/7 * 3 = 0.43)
            partial_b7 = {"status": "measured_partial", "total": 7, "checked": 1, "clean": 1, "unavailable_count": 6, "violations": []}
            cons_p = C.score_consistency({"kinds": {"full": {"b7_checks": partial_b7}}})
            self.assertAlmostEqual(cons_p["B7"]["points"], round(3.0 / 7.0, 2))
            self.assertEqual(cons_p["B7"]["unavailable_count"], 6)

    def test_parse_client_log_strict_bool_drag_ok_and_hub_layout_leaves(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            log = Path(td) / "client.log"
            log.write_text("\n".join([
                'GPUI_METRIC {"event":"mark","name":"b7:resize-drag"}',
                'GPUI_METRIC {"event":"tiles","tiles":[{"id":"t0","kind":"board","source":"a.kicad_pcb","rect":{"x":753,"y":36,"w":300,"h":800},"body":{"x":753,"y":68,"w":300,"h":768},"frame":2,"focused":true}],"splitters":[{"id":"sp0","rect":{"x":1053,"y":36,"w":4,"h":800},"axis":"x"}]}',
                'GPUI_METRIC {"event":"drag","ok":"false","splitter":"sp0","from":[1055,400],"to":[980,400],"moves":4}',
                'GPUI_METRIC {"event":"shot-ready","name":"b7-after-resize","state":"ready","settled":true,"viewer":{"open":"board"}}',
                'GPUI_METRIC {"event":"mark","name":"b7:overlay-incompatible"}',
                'GPUI_METRIC {"event":"refusal","code":"incompatibleOverlay","text":"Those views cannot be overlaid"}',
            ]) + "\n", encoding="utf-8")
            parsed = P.parse_client_log(log)
            self.assertTrue(parsed["tiles_metric_seen"])
            # String "false" MUST be parsed as False (strict bool)
            self.assertIs(parsed["drags"]["b7:resize-drag"][0]["ok"], False)
            self.assertGreater(parsed["shots"]["b7-after-resize"]["tiles_seq"], parsed["mark_seqs"]["b7:resize-drag"])
            self.assertEqual(parsed["refusals"]["b7:overlay-incompatible"][0]["code"], "incompatibleOverlay")

            # Hub layout.root leaves + row/col -> x/y translation
            hub_layout = {
                "root": {
                    "split": {
                        "axis": "row",
                        "a": {"leaf": {"tile": {"id": "tile-a", "kind": "gerbers", "source": "mfg/gerbers"}}},
                        "b": {"leaf": {"tile": {"id": "tile-b", "kind": "board", "source": "ecc83-pp.kicad_pcb"}}},
                    }
                }
            }
            leaves = DOCK.layout_leaves(hub_layout)
            self.assertEqual(set(leaves.keys()), {"tile-a", "tile-b"})
            self.assertEqual(DOCK.axis_from_hub("row"), "x")
            self.assertEqual(DOCK.axis_from_hub("col"), "y")

    def test_normal_driver_schedules_dock_steps_without_self_closing_board(self) -> None:
        steps = D.build_script("full", 300, 15000, embed=False)
        marks = [s.get("mark") for s in steps if isinstance(s, dict) and "mark" in s]
        self.assertIn("press:task4-dock-gerbers", marks)
        self.assertIn("press:task4-dock-split", marks)
        self.assertIn("press:task4-dock-back", marks)
        for side in ("right", "left", "bottom", "top"):
            self.assertIn(f"b7:add-{side}", marks)
        self.assertIn("b7:swap", marks)
        self.assertIn("b7:resize-drag", marks)
        self.assertIn("b7:maximize", marks)
        self.assertIn("b7:restore", marks)
        self.assertIn("b7:overlay-on", marks)
        self.assertIn("b7:overlay-off", marks)
        self.assertIn("b7:overlay-incompatible", marks)
        # Task 4 uses user-facing UI controls, not direct layout-add
        t4_start = marks.index("press:task4-dock-split")
        t4_steps = [s for s in steps if isinstance(s, dict)]
        idx_mark = next(i for i, s in enumerate(t4_steps) if s.get("mark") == "press:task4-dock-split")
        self.assertEqual(t4_steps[idx_mark + 1].get("press"), "ui:Add beside")
        # Between direction trials, idle reset occurs before pressing rail:Board
        idx_left_idle = next(i for i, s in enumerate(t4_steps) if s.get("tag") == "b7-add-left-idle")
        self.assertEqual(t4_steps[idx_left_idle + 1].get("press"), "rail:Board")

    def test_task4_dock_passes_and_catches_stale_identical_missing_start_and_privacy(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            lay = {"topbar": {"x": 0, "y": 0, "w": 1440, "h": 36}, "main": {"x": 264, "y": 36, "w": 488, "h": 835}, "rail": {"x": 1369, "y": 36, "w": 71, "h": 835}}
            r_full = {"x": 753, "y": 36, "w": 616, "h": 835}
            r_left = {"x": 753, "y": 36, "w": 306, "h": 835}
            r_right = {"x": 1063, "y": 36, "w": 306, "h": 835}
            sp_x = {"id": "sp-1", "rect": {"x": 1059, "y": 36, "w": 4, "h": 835}, "axis": "x"}

            _write_painted_png(fdir / "task4-dock-gerbers.png", [(r_full, (60, 180, 120), True)])
            _write_painted_png(fdir / "task4-dock-split.png", [(r_left, (60, 180, 120), True), (r_right, (200, 160, 60), True)])
            _write_painted_png(fdir / "task4-dock-back.png", [(r_full, (60, 180, 120), True)])

            parsed = {
                "tiles_metric_seen": True,
                "mark_seqs": {"press:task4-dock-gerbers": 10, "press:task4-dock-split": 20, "press:task4-dock-back": 30},
                "presses": {
                    "press:task4-dock-gerbers": [{"id": "rail:Gerbers", "found": True, "seq": 11}],
                    "press:task4-dock-split": [{"id": "ui:Add beside", "found": True, "seq": 21}, {"id": "add-beside:board", "found": True, "seq": 22}],
                    "press:task4-dock-back": [{"id": "tile-close:board", "found": True, "seq": 31}],
                },
                "shots": {
                    "task4-dock-gerbers": {
                        "state": "ready", "layout": lay, "tiles_seq": 15,
                        "tiles": [{"id": "tile-g", "kind": "gerbers", "source": "mfg/gerbers", "rect": r_full, "body": _body_of(r_full), "frame": 1, "focused": True}],
                        "splitters": [], "viewer": {},
                    },
                    "task4-dock-split": {
                        "state": "ready", "layout": lay, "tiles_seq": 25,
                        "tiles": [
                            {"id": "tile-g", "kind": "gerbers", "source": "mfg/gerbers", "rect": r_left, "body": _body_of(r_left), "frame": 2, "focused": False},
                            {"id": "tile-b", "kind": "board", "source": "ecc83-pp.kicad_pcb", "rect": r_right, "body": _body_of(r_right), "frame": 1, "focused": True},
                        ],
                        "splitters": [sp_x], "viewer": {},
                    },
                    "task4-dock-back": {
                        "state": "ready", "layout": lay, "tiles_seq": 35,
                        "tiles": [{"id": "tile-g", "kind": "gerbers", "source": "mfg/gerbers", "rect": r_full, "body": _body_of(r_full), "frame": 3, "focused": True}],
                        "splitters": [], "viewer": {},
                    },
                },
            }
            caps = {"task4-dock-gerbers": _ok_cap(), "task4-dock-split": _ok_cap(), "task4-dock-back": _ok_cap()}
            cleared = set(caps.keys())

            # 1. Clean pass
            res = DOCK.evaluate_task4_dock(fdir, parsed, caps, True, cleared_shots=cleared)
            self.assertEqual(res["outcome"], "passed", res)
            self.assertTrue(res["one_step_back"])

            # 2. Stale inventory (tiles_seq <= mark_seq) -> unavailable
            p_stale = json.loads(json.dumps(parsed))
            p_stale["shots"]["task4-dock-split"]["tiles_seq"] = 19
            res_stale = DOCK.evaluate_task4_dock(fdir, p_stale, caps, True, cleared_shots=cleared)
            self.assertEqual(res_stale["outcome"], "unavailable")
            self.assertIn("stale", res_stale["note"])

            # 3. Identical bodies in pixels -> task_failed
            _write_painted_png(fdir / "task4-dock-split.png", [(r_left, (60, 180, 120), True), (r_right, (60, 180, 120), True)])
            res_ident = DOCK.evaluate_task4_dock(fdir, parsed, caps, True, cleared_shots=cleared)
            self.assertEqual(res_ident["outcome"], "task_failed")
            self.assertIn("differ visually", res_ident["note"])
            _write_painted_png(fdir / "task4-dock-split.png", [(r_left, (60, 180, 120), True), (r_right, (200, 160, 60), True)])

            # 4. Missing starting Gerbers shot -> unavailable (never passed)
            p_nostart = json.loads(json.dumps(parsed))
            del p_nostart["shots"]["task4-dock-gerbers"]
            self.assertEqual(DOCK.evaluate_task4_dock(fdir, p_nostart, caps, True, cleared_shots=cleared)["outcome"], "unavailable")

            # 5. Not in per-shot privacy clearance -> harness_failed (fail-closed)
            self.assertEqual(DOCK.evaluate_task4_dock(fdir, parsed, caps, True, cleared_shots={"task4-dock-gerbers"})["outcome"], "harness_failed")

    def test_b7_all_seven_checks_and_missing_pane_geometry_and_stale_and_string_false(self) -> None:
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
            sp_x = {"id": "sp-x", "rect": {"x": 1059, "y": 36, "w": 4, "h": 835}, "axis": "x"}
            sp_x_moved = {"id": "sp-x", "rect": {"x": 973, "y": 36, "w": 4, "h": 835}, "axis": "x"}
            sp_y = {"id": "sp-y", "rect": {"x": 753, "y": 451, "w": 616, "h": 4}, "axis": "y"}

            c_brd, c_sch, c_ov = (200, 160, 60), (80, 160, 230), (230, 80, 190)
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
            _write_painted_png(fdir / "b7-overlay-incompat-before.png", [(r_l, c_brd, True), (r_r, c_sch, True)])
            _write_painted_png(fdir / "b7-overlay-incompat.png", [(r_l, c_brd, True), (r_r, c_sch, True)])

            def _t(tid, kind, src, rect, frame, overlay=...):
                d = {"id": tid, "kind": kind, "source": src, "rect": rect, "body": _body_of(rect), "frame": frame}
                if overlay is not ...:
                    d["overlay"] = overlay
                return d

            inc_layout = {"root": {"split": {"axis": "row", "a": {"leaf": {"tile": {"id": "tb", "kind": "board"}}}, "b": {"leaf": {"tile": {"id": "tf", "kind": "files"}}}}}}
            mark_seqs = {
                "b7:add-right": 10, "b7:add-left": 20, "b7:add-bottom": 30, "b7:add-top": 40,
                "b7:close-right": 50, "b7:swap": 60, "b7:resize-drag": 70,
                "b7:maximize": 80, "b7:restore": 90, "b7:overlay-on": 100, "b7:overlay-off": 110,
                "b7:overlay-incompatible": 120,
            }
            shots = {
                "b7-add-right": {"layout": lay, "tiles_seq": 15, "tiles": [_t("tb", "board", "b.kicad_pcb", r_l, 1), _t("ts", "schematic", "s.kicad_sch", r_r, 1)], "splitters": [sp_x], "viewer": {}},
                "b7-add-left": {"layout": lay, "tiles_seq": 25, "tiles": [_t("ts", "schematic", "s.kicad_sch", r_l, 1), _t("tb", "board", "b.kicad_pcb", r_r, 1)], "splitters": [sp_x], "viewer": {}},
                "b7-add-bottom": {"layout": lay, "tiles_seq": 35, "tiles": [_t("tb", "board", "b.kicad_pcb", r_t, 1), _t("ts", "schematic", "s.kicad_sch", r_b, 1)], "splitters": [sp_y], "viewer": {}},
                "b7-add-top": {"layout": lay, "tiles_seq": 45, "tiles": [_t("ts", "schematic", "s.kicad_sch", r_t, 1), _t("tb", "board", "b.kicad_pcb", r_b, 1)], "splitters": [sp_y], "viewer": {}},
                "b7-close-right": {"layout": lay, "tiles_seq": 55, "tiles": [_t("tb", "board", "b.kicad_pcb", r_full, 2)], "splitters": [], "viewer": {}},
                "b7-before-swap": {"layout": lay, "tiles_seq": 58, "tiles": [_t("tb", "board", "b.kicad_pcb", r_l, 2), _t("ts", "schematic", "s.kicad_sch", r_r, 2)], "splitters": [sp_x], "viewer": {}},
                "b7-after-swap": {"layout": lay, "tiles_seq": 65, "tiles": [_t("ts", "schematic", "s.kicad_sch", r_l, 3), _t("tb", "board", "b.kicad_pcb", r_r, 3)], "splitters": [sp_x], "viewer": {}},
                "b7-after-resize": {"layout": lay, "tiles_seq": 75, "tiles": [_t("ts", "schematic", "s.kicad_sch", r_l_narrow, 4), _t("tb", "board", "b.kicad_pcb", r_r_wide, 4)], "splitters": [sp_x_moved], "viewer": {}},
                "b7-maximized": {"layout": lay, "tiles_seq": 85, "tiles": [_t("ts", "schematic", "s.kicad_sch", r_full, 5)], "splitters": [], "viewer": {}},
                "b7-restored": {"layout": lay, "tiles_seq": 95, "tiles": [_t("ts", "schematic", "s.kicad_sch", r_l_narrow, 6), _t("tb", "board", "b.kicad_pcb", r_r_wide, 6)], "splitters": [sp_x_moved], "viewer": {}},
                "b7-overlay-plain": {"layout": lay, "tiles_seq": 98, "tiles": [_t("tb", "board", "b.kicad_pcb", r_full, 7, overlay=None)], "splitters": [], "viewer": {}},
                "b7-overlay-on": {"layout": lay, "tiles_seq": 105, "tiles": [_t("tb", "board", "b.kicad_pcb", r_full, 8, overlay={"kind": "gerbers"})], "splitters": [], "viewer": {}},
                "b7-overlay-off": {"layout": lay, "tiles_seq": 115, "tiles": [_t("tb", "board", "b.kicad_pcb", r_full, 9, overlay=None)], "splitters": [], "viewer": {}},
                "b7-overlay-incompat-before": {"layout": lay, "tiles_seq": 118, "tiles": [_t("tb", "board", "b.kicad_pcb", r_l, 10), _t("tf", "files", "Project", r_r, 1)], "splitters": [sp_x], "viewer": {"layout": inc_layout}},
                "b7-overlay-incompat": {"layout": lay, "tiles_seq": 125, "tiles": [_t("tb", "board", "b.kicad_pcb", r_l, 10), _t("tf", "files", "Project", r_r, 1)], "splitters": [sp_x], "viewer": {"layout": inc_layout}},
            }
            caps = {k: _ok_cap() for k in shots}
            cleared = set(shots.keys())
            parsed = {
                "tiles_metric_seen": True,
                "mark_seqs": mark_seqs,
                "shots": shots,
                "drags": {"b7:resize-drag": [{"ok": True, "splitter": "sp-x", "from": [1061, 420], "to": [975, 420], "moves": 6}]},
                "refusals": {"b7:overlay-incompatible": [{"code": "incompatibleOverlay", "text": "Those views cannot be overlaid"}]},
            }

            b7 = DOCK.evaluate_b7_dock(fdir, parsed, caps, cleared_shots=cleared)
            self.assertEqual(b7["status"], "measured", b7)
            self.assertEqual(b7["clean"], 7)

            # Missing pane geometry on close & maximize -> unavailable (never waived!)
            p_nogeom = json.loads(json.dumps(parsed))
            p_nogeom["shots"]["b7-close-right"]["layout"] = {}
            p_nogeom["shots"]["b7-maximized"]["layout"] = {}
            b7_ng = DOCK.evaluate_b7_dock(fdir, p_nogeom, caps, cleared_shots=cleared)
            self.assertEqual(b7_ng["checks"]["close"]["status"], "unavailable")
            self.assertEqual(b7_ng["checks"]["maximize_restore"]["status"], "unavailable")

            # String "false" in drag.ok -> resize_drag unavailable
            p_strfalse = json.loads(json.dumps(parsed))
            p_strfalse["drags"]["b7:resize-drag"] = [{"ok": "false", "splitter": "sp-x", "from": [1061, 420], "to": [975, 420]}]
            b7_sf = DOCK.evaluate_b7_dock(fdir, p_strfalse, caps, cleared_shots=cleared)
            self.assertEqual(b7_sf["checks"]["resize_drag"]["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
