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


def _header_controls(tiles: list[dict]) -> dict:
    """What a healthy build measures for each tile: a visible close control inside the tile's header band (between its rect top and its body top)."""
    out = {}
    for t in tiles:
        r, b = t.get("rect"), t.get("body")
        if not (isinstance(r, dict) and isinstance(b, dict)):
            continue
        out[f"tile:{t['id']}:close"] = {"rect": {"x": r["x"] + r["w"] - 30, "y": r["y"] + 4, "w": 24, "h": max(b["y"] - r["y"] - 8, 1)}, "visible": True, "enabled": True}
    return out


def _with_controls(parsed: dict) -> dict:
    """The parsed client log with every shot's tile header controls added where the fixture names none (a copy)."""
    p = json.loads(json.dumps(parsed))
    for shot in (p.get("shots") or {}).values():
        if "tile_controls" not in shot:
            shot["tile_controls"] = _header_controls(shot.get("tiles") or [])
    return p


def _t4(full_dir, parsed, *a, **kw):
    return DOCK.evaluate_task4_dock(full_dir, _with_controls(parsed), *a, **kw)


def _b7(full_dir, parsed, *a, **kw):
    return DOCK.evaluate_b7_dock(full_dir, _with_controls(parsed), *a, **kw)


class RailDockHarnessTests(unittest.TestCase):
    def test_unavailable_without_tiles_metric_and_b7_scored_over_all_seven(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            parsed = {"tiles_metric_seen": False, "shots": {}}
            self.assertIsNone(_t4(fdir, parsed, {}, True))
            b7 = _b7(fdir, parsed, {})
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
            self.assertFalse(parsed["refusal_metric_declared"])  # the event did not declare `refusal_metric`
            # String "false" MUST be parsed as False (strict bool)
            self.assertIs(parsed["drags"]["b7:resize-drag"][0]["ok"], False)
            self.assertGreater(parsed["shots"]["b7-after-resize"]["tiles_seq"], parsed["mark_seqs"]["b7:resize-drag"])
            self.assertEqual(parsed["refusals"]["b7:overlay-incompatible"][0]["code"], "incompatibleOverlay")

            # The hub's real shape (present/layout.rs): `{"type":"tile",...}` leaves and `{"type":"split","axis":"row"|"col","a","b"}`
            hub_layout = {
                "version": 1,
                "root": {"type": "split", "id": "s1", "axis": "row", "ratio": 0.5,
                         "a": {"type": "tile", "id": "tile-a", "kind": "gerbers", "source": "mfg/gerbers"},
                         "b": {"type": "split", "id": "s2", "axis": "col", "ratio": 0.4,
                               "a": {"type": "tile", "id": "tile-b", "kind": "board", "source": "ecc83-pp.kicad_pcb"},
                               "b": {"type": "tile", "id": "tile-c", "kind": "files", "source": ""}}},
                "focus": "tile-b",
            }
            leaves = DOCK.layout_leaves(hub_layout)
            self.assertEqual(set(leaves.keys()), {"tile-a", "tile-b", "tile-c"})
            self.assertEqual(DOCK.layout_splits(hub_layout), {"s1": {"axis": "x", "ratio": 0.5}, "s2": {"axis": "y", "ratio": 0.4}})
            # Guessed shapes (nested leaf/tile wrappers, `viewer.layout.tiles`) earn nothing
            self.assertEqual(DOCK.layout_leaves({"root": {"split": {"axis": "row", "a": {"leaf": {"tile": {"id": "x"}}}}}}), {})
            self.assertEqual(DOCK.layout_leaves({"tiles": [{"id": "t0"}]}), {})
            self.assertEqual(DOCK._tile_source({"id": "tile-a"}, {"layout": hub_layout}), "mfg/gerbers")
            self.assertEqual(DOCK._tile_source({"id": "tile-a"}, {"tiles": {"tile-a": {"source": "guess"}}}), "")
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
        t4_steps = [s for s in steps if isinstance(s, dict)]
        idx_mark = next(i for i, s in enumerate(t4_steps) if s.get("mark") == "press:task4-dock-split")
        # Task 4 (budget two) is a physical Shift+click on the rail's Board row after rail:Gerbers; no alias, no direct hub layout action anywhere
        self.assertEqual(t4_steps[idx_mark + 1], {"pointer": "rail:Board", "modifier": "shift"})
        # the menu route is a separate measured route with its own marks: Add beside on the drawn Gerbers tile, then the Board entry
        idx_menu = next(i for i, s in enumerate(t4_steps) if s.get("mark") == "press:task4-menu-split")
        self.assertEqual(t4_steps[idx_menu + 1].get("press"), "@tile[kind=gerbers].add-beside")
        self.assertEqual(t4_steps[idx_menu + 3].get("press"), "add-beside:board")
        self.assertIn("press:task4-menu-gerbers", marks)
        self.assertIn("press:task4-menu-back", marks)
        self.assertFalse([s for s in t4_steps if "action" in s and str(s["action"][0]).startswith("layout")])
        # No literal tile/splitter ids: every tile or splitter target is a selector resolved from the drawn inventory
        for s in t4_steps:
            for key in ("press", "click"):
                v = str(s.get(key) or "")
                self.assertNotRegex(v, r"^(tile-close|tile-splitter|splitter):", s)
        drag = next(s for s in t4_steps if "drag" in s)
        self.assertEqual(drag["drag"]["splitter"], "@splitter[axis=x]")
        # Each close/refusal leaves Board visible. A second Board press would close it.
        start = next(i for i,s in enumerate(t4_steps) if s.get("tag") == "b7-add-right-idle")
        end = next(i for i,s in enumerate(t4_steps) if s.get("mark") == "b7:pair")
        # the directions start from two actual tiles by the genuine route (rail:Gerbers, then ONE physical Shift+click on rail:Board): no plain press of rail:Board
        # (a second one would close it), and each close/refusal leaves the two tiles
        self.assertEqual([s["press"] for s in t4_steps[start:end] if s.get("press") == "rail:Board"], [])
        self.assertEqual([s for s in t4_steps[start:end] if s.get("pointer") == "rail:Board"], [{"pointer": "rail:Board", "modifier": "shift"}])
        for side in ("right", "left", "bottom", "top"):
            self.assertIn({"mark": f"b7:add-{side}"}, t4_steps[start:end])
        # the other kinds are still scheduled (the elif chain around the dock block stayed intact)
        self.assertTrue(D.build_script("schematic-only", 300, 15000, embed=True))
        self.assertFalse([s for s in D.build_script("mech-only", 300, 15000) if "mark" in s and str(s["mark"]).startswith("b7:")])

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
                    "press:task4-dock-split": [{"id": "tile:tile-g:add-beside", "found": True, "seq": 21}, {"id": "add-beside:board", "found": True, "seq": 22}],
                    "press:task4-dock-back": [{"id": "tile:tile-b:close", "found": True, "seq": 31}],
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
            res = _t4(fdir, parsed, caps, True, cleared_shots=cleared)
            self.assertEqual(res["outcome"], "passed", res)
            self.assertTrue(res["one_step_back"])

            # 1b. A usable tile is more than a frame count: the pane's Gerbers tile collapsed to a sliver (the light-minimum capture) is task_failed, an unmeasured
            # header is unavailable, never passed
            p_sliver = json.loads(json.dumps(parsed))
            g_t = p_sliver["shots"]["task4-dock-split"]["tiles"][0]
            g_t["rect"] = {**g_t["rect"], "w": 8.0}
            g_t["body"] = {**g_t["body"], "w": 8.0}
            res_sliver = _t4(fdir, p_sliver, caps, True, cleared_shots=cleared)
            self.assertEqual(res_sliver["outcome"], "task_failed", res_sliver)
            self.assertIn("minimum", res_sliver["note"])
            p_nohdr = json.loads(json.dumps(parsed))
            for shot in p_nohdr["shots"].values():
                shot["tile_controls"] = None
            self.assertEqual(DOCK.evaluate_task4_dock(fdir, p_nohdr, caps, True, cleared_shots=cleared)["outcome"], "unavailable")
            p_out = json.loads(json.dumps(parsed))
            p_out["shots"]["task4-dock-split"]["tile_controls"] = {"tile:tile-g:close": {"rect": {"x": 5000.0, "y": 40.0, "w": 24.0, "h": 24.0}, "visible": True}, **_header_controls([p_out["shots"]["task4-dock-split"]["tiles"][1]])}
            res_out = DOCK.evaluate_task4_dock(fdir, p_out, caps, True, cleared_shots=cleared)
            self.assertEqual(res_out["outcome"], "task_failed", res_out)
            self.assertIn("header control", res_out["note"])

            # 2. Stale inventory (tiles_seq <= mark_seq) -> unavailable
            p_stale = json.loads(json.dumps(parsed))
            p_stale["shots"]["task4-dock-split"]["tiles_seq"] = 19
            res_stale = _t4(fdir, p_stale, caps, True, cleared_shots=cleared)
            self.assertEqual(res_stale["outcome"], "unavailable")
            self.assertIn("stale", res_stale["note"])

            # 3. Identical bodies in pixels -> task_failed
            _write_painted_png(fdir / "task4-dock-split.png", [(r_left, (60, 180, 120), True), (r_right, (60, 180, 120), True)])
            res_ident = _t4(fdir, parsed, caps, True, cleared_shots=cleared)
            self.assertEqual(res_ident["outcome"], "task_failed")
            self.assertIn("differ visually", res_ident["note"])
            _write_painted_png(fdir / "task4-dock-split.png", [(r_left, (60, 180, 120), True), (r_right, (200, 160, 60), True)])

            # 4. Missing starting Gerbers shot -> unavailable (never passed)
            p_nostart = json.loads(json.dumps(parsed))
            del p_nostart["shots"]["task4-dock-gerbers"]
            self.assertEqual(_t4(fdir, p_nostart, caps, True, cleared_shots=cleared)["outcome"], "unavailable")

            # 4b. Route not exact (an extra or reordered press), or the wrong tile's control -> unavailable, never passed
            for bad in (
                [{"id": "add-beside:board", "found": True, "seq": 22}, {"id": "tile:tile-g:add-beside", "found": True, "seq": 21}],
                [{"id": "tile:tile-b:add-beside", "found": True, "seq": 21}, {"id": "add-beside:board", "found": True, "seq": 22}],
                [{"id": "tile:tile-g:add-beside", "found": True, "seq": 21}],
                [{"id": "tile:tile-g:add-beside", "found": True, "seq": 21}, {"id": "add-beside:board", "found": True, "seq": 22}, {"id": "x", "found": True, "seq": 23}],
            ):
                p_bad = json.loads(json.dumps(parsed))
                p_bad["presses"]["press:task4-dock-split"] = bad
                self.assertEqual(_t4(fdir, p_bad, caps, True, cleared_shots=cleared)["outcome"], "unavailable")
            # 4c. Outer rect missing while the body is present -> unavailable (not a product failure)
            p_rect = json.loads(json.dumps(parsed))
            del p_rect["shots"]["task4-dock-split"]["tiles"][0]["rect"]
            self.assertEqual(_t4(fdir, p_rect, caps, True, cleared_shots=cleared)["outcome"], "unavailable")

            # 4d. A missing rect in the INITIAL or the RETURN shot is a width gap -> unavailable, never width_restored=False -> task_failed
            for shot in ("task4-dock-gerbers", "task4-dock-back"):
                p_w = json.loads(json.dumps(parsed))
                del p_w["shots"][shot]["tiles"][0]["rect"]
                self.assertEqual(_t4(fdir, p_w, caps, True, cleared_shots=cleared)["outcome"], "unavailable", shot)
            # ... while a measured narrow return width still fails
            p_narrow = json.loads(json.dumps(parsed))
            p_narrow["shots"]["task4-dock-back"]["tiles"][0]["rect"] = {"x": 753, "y": 36, "w": 300, "h": 835}
            self.assertEqual(_t4(fdir, p_narrow, caps, True, cleared_shots=cleared)["outcome"], "task_failed")

            # 5. Not in per-shot privacy clearance -> harness_failed (fail-closed)
            self.assertEqual(_t4(fdir, parsed, caps, True, cleared_shots={"task4-dock-gerbers"})["outcome"], "harness_failed")

    def test_task4_budget_two_route_steps_and_exact_pointer_chain(self) -> None:
        shift = DOCK.build_task4_dock_steps(route="shift")
        middle = DOCK.build_task4_dock_steps(route="middle")
        menu = DOCK.build_task4_dock_steps(route="menu")
        at = lambda st, mark: next(i for i, x in enumerate(st) if x.get("mark") == mark)  # noqa: E731
        self.assertEqual(shift[at(shift, "press:task4-dock-split") + 1], {"pointer": "rail:Board", "modifier": "shift"})
        self.assertEqual(middle[at(middle, "press:task4-dock-split") + 1], {"pointer": "rail:Board", "button": "middle"})
        # the scored route has two inputs between idle and the shot: rail:Gerbers and one pointer (never a press of rail:Board, never the menu)
        for st, pre in ((shift, "task4-dock"), (middle, "task4-dock")):
            body = st[at(st, f"press:{pre}-gerbers"):at(st, f"press:{pre}-back")]
            inputs = [x for x in body if "press" in x or "pointer" in x]
            self.assertEqual(len(inputs), 2, inputs)
            self.assertEqual(inputs[0], {"press": "rail:Gerbers"})
            self.assertFalse([x for x in body if "press" in x and "add-beside" in str(x["press"])])
        self.assertEqual([x.get("press") for x in menu if x.get("press") and x["press"] != "@tile[kind=board].close"][:1], ["rail:Gerbers"])
        self.assertNotIn({"pointer": "rail:Board", "modifier": "shift"}, menu)
        # the exact chain accepts the pointer only with the expected modifier and button, delivered, and never as a plain press of the same id
        ev = {"id": "rail:Board", "found": True, "pointer": True, "modifier": "shift", "button": "left"}
        want = DOCK._route_chain("shift", "g")
        self.assertEqual(DOCK._exact_chain({"m": [ev]}, "m", want), [ev])
        for bad in (
            {**ev, "modifier": "none"}, {**ev, "button": "middle"}, {**ev, "found": False}, {**ev, "pointer": False}, {**ev, "id": "rail:Gerbers"},
        ):
            self.assertEqual(DOCK._exact_chain({"m": [bad]}, "m", want), [], bad)
        mid = {"id": "rail:Board", "found": True, "pointer": True, "modifier": "none", "button": "middle"}
        self.assertEqual(DOCK._exact_chain({"m": [mid]}, "m", DOCK._route_chain("middle", "g")), [mid])
        self.assertEqual(DOCK._exact_chain({"m": [mid]}, "m", want), [], "a middle-click is not the Shift route")
        self.assertEqual(DOCK._exact_chain({"m": [ev, ev]}, "m", want), [], "an extra press is not the budget-two route")
        self.assertEqual(DOCK._exact_chain({"m": [{"id": "rail:Board", "found": True}]}, "m", ["rail:Board"]), [{"id": "rail:Board", "found": True}])
        self.assertEqual(DOCK._exact_chain({"m": [ev]}, "m", ["rail:Board"]), [], "a pointer is not a control press")
        with self.assertRaises(ValueError):
            DOCK._route_chain("keyboard", "g")

    def test_task4_modified_pointer_route_scores_and_the_menu_route_is_its_own_check(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            lay = {"topbar": {"x": 0, "y": 0, "w": 1440, "h": 36}, "main": {"x": 264, "y": 36, "w": 488, "h": 835}, "rail": {"x": 1369, "y": 36, "w": 71, "h": 835}}
            r_full = {"x": 753, "y": 36, "w": 616, "h": 835}
            r_left = {"x": 753, "y": 36, "w": 306, "h": 835}
            r_right = {"x": 1063, "y": 36, "w": 306, "h": 835}
            sp_x = {"id": "sp-1", "rect": {"x": 1059, "y": 36, "w": 4, "h": 835}, "axis": "x"}

            def route(pre: str, split_presses: list[dict]) -> tuple[dict, dict]:
                _write_painted_png(fdir / f"{pre}-gerbers.png", [(r_full, (60, 180, 120), True)])
                _write_painted_png(fdir / f"{pre}-split.png", [(r_left, (60, 180, 120), True), (r_right, (200, 160, 60), True)])
                _write_painted_png(fdir / f"{pre}-back.png", [(r_full, (60, 180, 120), True)])
                shots = {
                    f"{pre}-gerbers": {"state": "ready", "layout": lay, "tiles_seq": 15, "splitters": [], "viewer": {},
                        "tiles": [{"id": "tile-g", "kind": "gerbers", "source": "mfg/gerbers", "rect": r_full, "body": _body_of(r_full), "frame": 1, "focused": True}]},
                    f"{pre}-split": {"state": "ready", "layout": lay, "tiles_seq": 25, "splitters": [sp_x], "viewer": {}, "tiles": [
                        {"id": "tile-g", "kind": "gerbers", "source": "mfg/gerbers", "rect": r_left, "body": _body_of(r_left), "frame": 2, "focused": False},
                        {"id": "tile-b", "kind": "board", "source": "ecc83-pp.kicad_pcb", "rect": r_right, "body": _body_of(r_right), "frame": 1, "focused": True}]},
                    f"{pre}-back": {"state": "ready", "layout": lay, "tiles_seq": 35, "splitters": [], "viewer": {},
                        "tiles": [{"id": "tile-g", "kind": "gerbers", "source": "mfg/gerbers", "rect": r_full, "body": _body_of(r_full), "frame": 3, "focused": True}]},
                }
                parsed = {"tiles_metric_seen": True,
                          "mark_seqs": {f"press:{pre}-gerbers": 10, f"press:{pre}-split": 20, f"press:{pre}-back": 30},
                          "presses": {f"press:{pre}-gerbers": [{"id": "rail:Gerbers", "found": True, "seq": 11}],
                                      f"press:{pre}-split": split_presses,
                                      f"press:{pre}-back": [{"id": "tile:tile-b:close", "found": True, "seq": 31}]},
                          "shots": shots}
                return parsed, {k: _ok_cap() for k in shots}

            shift_ev = [{"id": "rail:Board", "found": True, "pointer": True, "modifier": "shift", "button": "left", "seq": 21}]
            parsed, caps = route("task4-dock", shift_ev)
            res = _t4(fdir, parsed, caps, True, cleared_shots=set(caps), route="shift")
            self.assertEqual(res["outcome"], "passed", res)
            self.assertEqual(res["presses"], ["rail:Gerbers", "rail:Board"], "two presses: within the budget of two")
            # the same inventory with the plain (unmodified) pointer is not the route: never a pass
            plain = [{**shift_ev[0], "modifier": "none"}]
            parsed_plain, _ = route("task4-dock", plain)
            self.assertEqual(_t4(fdir, parsed_plain, caps, True, cleared_shots=set(caps), route="shift")["outcome"], "unavailable")
            # the middle-click route is the alternate
            mid = [{**shift_ev[0], "modifier": "none", "button": "middle"}]
            parsed_mid, _ = route("task4-dock", mid)
            self.assertEqual(_t4(fdir, parsed_mid, caps, True, cleared_shots=set(caps), route="middle")["outcome"], "passed")
            # the menu route is measured on its own (three presses) and is an extra check, never the Task 4 score
            menu_ev = [{"id": "tile:tile-g:add-beside", "found": True, "seq": 21}, {"id": "add-beside:board", "found": True, "seq": 22}]
            parsed_menu, caps_menu = route("task4-menu", menu_ev)
            chk = DOCK.task4_menu_route_check(fdir, _with_controls(parsed_menu), caps_menu, True, cleared_shots=set(caps_menu))
            self.assertEqual((chk["id"], chk["outcome"]), ("full/task4-menu-route", "passed"), chk)
            self.assertEqual(chk["evidence"]["presses"], ["rail:Gerbers", "tile:tile-g:add-beside", "add-beside:board"])
            # the folded row of the More menu is as good a first input as the visible button; a folded row of ANOTHER tile, or a pointer, is not
            folded = [{**menu_ev[0], "id": "tile:tile-g:more:add-beside"}, menu_ev[1]]
            parsed_fold, caps_fold = route("task4-menu", folded)
            self.assertEqual(DOCK.task4_menu_route_check(fdir, _with_controls(parsed_fold), caps_fold, True, cleared_shots=set(caps_fold))["outcome"], "passed")
            for bad in ([{**menu_ev[0], "id": "tile:tile-b:more:add-beside"}, menu_ev[1]], [{**menu_ev[0], "pointer": True}, menu_ev[1]]):
                parsed_x, caps_x = route("task4-menu", bad)
                self.assertEqual(DOCK.task4_menu_route_check(fdir, _with_controls(parsed_x), caps_x, True, cleared_shots=set(caps_x))["outcome"], "unavailable")
            self.assertEqual(DOCK._route_chain("menu", "g"), [["tile:g:add-beside", "tile:g:more:add-beside"], "add-beside:board"])
            parsed_bad, caps_bad = route("task4-menu", [menu_ev[0]])
            self.assertEqual(DOCK.task4_menu_route_check(fdir, _with_controls(parsed_bad), caps_bad, True, cleared_shots=set(caps_bad))["outcome"], "unavailable")
            # a run without the menu route has no such check
            self.assertIsNone(DOCK.task4_menu_route_check(fdir, _with_controls(parsed), caps, True, cleared_shots=set(caps)))

    def test_pointer_metric_modifier_and_button_reach_the_press_records(self) -> None:
        ev = {"event": "pointer", "id": "rail:Board", "modifier": "shift", "button": "left", "hit_rect": True, "dispatched": True, "x": 1.0, "y": 2.0, "error": None}
        with tempfile.TemporaryDirectory() as td:
            log = Path(td) / "client.log"
            log.write_text("GPUI_METRIC " + json.dumps({"event": "mark", "name": "press:t"}) + "\nGPUI_METRIC " + json.dumps(ev) + "\n", encoding="utf-8")
            parsed = P.parse_client_log(log)
            p = parsed["presses"]["press:t"][0]
        self.assertEqual((p["modifier"], p["button"], p["found"], p["pointer"]), ("shift", "left", True, True))

    ORIGINAL_TILES = [
        {"id": "t1", "kind": "gerbers", "source": "mfg/gerbers", "overlay": None, "focused": False, "rect": {"x": 406.0, "y": 46.0, "w": 8.0, "h": 388.0}, "body": {"x": 406.0, "y": 86.0, "w": 8.0, "h": 348.0}, "frame": 14},
        {"id": "t2", "kind": "board", "source": "ecc83-pp.kicad_pcb", "overlay": None, "focused": True, "rect": {"x": 406.0, "y": 444.0, "w": 317.0, "h": 388.0}, "body": {"x": 406.0, "y": 484.0, "w": 317.0, "h": 348.0}, "frame": 14},
    ]
    ORIGINAL_SPLITTERS = [{"id": "s1", "rect": {"x": 437.0, "y": 433.0, "w": 287.0, "h": 12.0}, "axis": "y"}]
    # the controls of the original capture (light-minimum-painted): Gerbers' header controls drawn at x 410..489, outside its 8 px rect
    ORIGINAL_CONTROLS = {
        "tile:t1:add-beside": {"rect": {"x": 432.0, "y": 51.0, "w": 30.0, "h": 30.0}, "visible": True},
        "tile:t1:close": {"rect": {"x": 464.0, "y": 49.0, "w": 25.0, "h": 33.0}, "visible": True},
        "tile:t1:focus": {"rect": {"x": 410.0, "y": 49.0, "w": 18.0, "h": 33.0}, "visible": True},
        "tile:t2:close": {"rect": {"x": 694.0, "y": 447.0, "w": 25.0, "h": 33.0}, "visible": True},
        "tile:t2:focus": {"rect": {"x": 410.0, "y": 447.0, "w": 18.0, "h": 33.0}, "visible": True},
    }

    def test_a_tile_collapsed_to_a_sliver_is_not_a_usable_tile_even_with_frames_and_a_body(self) -> None:
        fails, missing = DOCK.usable_tile_fails(self.ORIGINAL_TILES, self.ORIGINAL_SPLITTERS, self.ORIGINAL_CONTROLS)
        self.assertEqual(missing, [])
        text = "; ".join(fails)
        for need in ("cross-axis span 8px", "width 8px is under the 180px minimum", "no measured header control"):
            self.assertIn(need, text)
        # the fixed build: both tiles span 317 px and every header control sits in its own header
        fixed = json.loads(json.dumps(self.ORIGINAL_TILES))
        fixed[0]["rect"]["w"] = fixed[0]["body"]["w"] = 317.0
        controls = {"tile:t1:close": {"rect": {"x": 690.0, "y": 49.0, "w": 25.0, "h": 33.0}, "visible": True}, "tile:t2:close": self.ORIGINAL_CONTROLS["tile:t2:close"]}
        self.assertEqual(DOCK.usable_tile_fails(fixed, self.ORIGINAL_SPLITTERS, controls), ([], []))
        # an entry of the More menu is not a header control, even drawn inside the band; the visible glyph is
        only_more = {**controls, "tile:t1:close": {"rect": {"x": 690.0, "y": 49.0, "w": 25.0, "h": 33.0}, "visible": False},
                     "tile:t1:more:add-beside": {"rect": {"x": 690.0, "y": 49.0, "w": 25.0, "h": 33.0}, "visible": True}}
        self.assertTrue(DOCK.usable_tile_fails(fixed, self.ORIGINAL_SPLITTERS, only_more)[0])
        glyph = {**controls, "tile:t1:close": {"rect": {"x": 690.0, "y": 49.0, "w": 25.0, "h": 33.0}, "visible": False},
                 "tile:t1:add-beside": {"rect": {"x": 660.0, "y": 49.0, "w": 25.0, "h": 33.0}, "visible": True}}
        self.assertEqual(DOCK.usable_tile_fails(fixed, self.ORIGINAL_SPLITTERS, glyph), ([], []))
        # a tile whose header controls are not measured is unmeasured, never a pass
        fails, missing = DOCK.usable_tile_fails(fixed, self.ORIGINAL_SPLITTERS, None)
        self.assertEqual(fails, [])
        self.assertTrue(missing)
        # a hidden or zero-size control is not a drawn one
        hidden = {**controls, "tile:t1:close": {"rect": {"x": 690.0, "y": 49.0, "w": 0.0, "h": 33.0}, "visible": True}}
        self.assertTrue(DOCK.usable_tile_fails(fixed, self.ORIGINAL_SPLITTERS, hidden)[0])
        # side by side: the height is the shared span; a tile under the minimum in either direction fails
        side = [{"id": "a", "rect": {"x": 0.0, "y": 0.0, "w": 300.0, "h": 400.0}, "body": {"x": 0.0, "y": 40.0, "w": 300.0, "h": 360.0}},
                {"id": "b", "rect": {"x": 308.0, "y": 0.0, "w": 120.0, "h": 400.0}, "body": {"x": 308.0, "y": 40.0, "w": 120.0, "h": 360.0}}]
        sc = {f"tile:{i}:close": {"rect": {"x": r + 4, "y": 4.0, "w": 20.0, "h": 30.0}, "visible": True} for i, r in (("a", 0.0), ("b", 308.0))}
        fails, _ = DOCK.usable_tile_fails(side, [{"id": "s", "axis": "x", "rect": {"x": 300.0, "y": 0.0, "w": 8.0, "h": 400.0}}], sc)
        self.assertTrue(any("tile b width 120px" in f for f in fails), fails)
        # one tile is nothing to compare
        self.assertEqual(DOCK.usable_tile_fails(side[:1], [], None), ([], []))

    def test_the_tile_header_controls_reach_the_shot_record(self) -> None:
        ev = {"event": "controls", "controls": [{"id": "tile:t1:close", "label": "Close", "rect": {"x": 464.0, "y": 49.0, "w": 25.0, "h": 33.0}, "enabled": True, "visible": True},
                                               {"id": "rail:Board", "label": "Board", "rect": {"x": 727.0, "y": 53.0, "w": 69.0, "h": 60.0}, "enabled": True, "visible": True}]}
        with tempfile.TemporaryDirectory() as td:
            log = Path(td) / "client.log"
            log.write_text("GPUI_METRIC " + json.dumps(ev) + "\nGPUI_METRIC " + json.dumps({"event": "shot-ready", "name": "b7-add-bottom", "state": "ready", "settled": True, "viewer": {}}) + "\n", encoding="utf-8")
            rec = P.parse_client_log(log)["shots"]["b7-add-bottom"]
            self.assertEqual(sorted(rec["tile_controls"]), ["tile:t1:close"])
            self.assertEqual(rec["tile_controls"]["tile:t1:close"]["rect"]["x"], 464.0)
            log.write_text("GPUI_METRIC " + json.dumps({"event": "shot-ready", "name": "b7-add-bottom", "state": "ready", "settled": True, "viewer": {}}) + "\n", encoding="utf-8")
            self.assertIsNone(P.parse_client_log(log)["shots"]["b7-add-bottom"]["tile_controls"], "no controls measurement at all")

    def test_b7_palette_route_steps_read_the_picker_before_enter(self) -> None:
        pal = DOCK._add_beside("kind=board", "schematic", "right", via="palette")
        self.assertEqual([next(iter(s.items())) for s in pal[:7]],
                         [("press", "rail:Commands"), ("wait", 120), ("keys", "a d d space b e s i d e"), ("wait", 120), ("read", "palette"), ("keys", "enter"), ("wait", 150)])
        self.assertEqual(pal[7], {"press": "add-beside:schematic:right"})
        self.assertEqual(DOCK._add_beside("kind=board", "schematic", via="palette")[7], {"press": "add-beside:schematic"})
        self.assertEqual(DOCK._add_beside("kind=board", "schematic", "left")[0], {"press": "@tile[kind=board].add-beside"})
        self.assertEqual(DOCK._add_chain("t1", "schematic", "top", "palette"), [["rail:Commands"], "add-beside:schematic:top"])
        self.assertEqual(DOCK._add_chain("t1", "schematic", None, "header"), [["tile:t1:add-beside", "tile:t1:more:add-beside"], "add-beside:schematic"])
        self.assertEqual(DOCK._add_via({"m": [{"id": "rail:Commands", "found": True}]}, "m"), "palette")
        self.assertEqual(DOCK._add_via({"m": [{"id": "tile:t1:add-beside", "found": True}]}, "m"), "header")
        # the More route (the tile's physical More button, then its Add beside row) at EVERY width; the palette only by RAIL_B7_VIA (no hub action in either)
        import os
        for size, first in (("", {"press": "@tile[kind=board].more"}), ("minimum", {"press": "@tile[kind=board].more"})):
            old = os.environ.get("RAIL_WINDOW_SIZE")
            os.environ["RAIL_WINDOW_SIZE"] = size
            try:
                steps = DOCK.build_b7_steps()
            finally:
                os.environ.pop("RAIL_WINDOW_SIZE", None) if old is None else os.environ.__setitem__("RAIL_WINDOW_SIZE", old)
            i = next(k for k, x in enumerate(steps) if x.get("mark") == "b7:add-right")
            self.assertEqual(steps[i + 1], first, size)
            self.assertFalse([x for x in steps if "action" in x])
        self.assertEqual(DOCK.palette_selected_add_beside({"palette_reads": {"m": [{"supported": True, "snapshot": {"open": True, "selected_id": "add-beside"}}]}}, "m"), "")
        self.assertTrue(DOCK.palette_selected_add_beside({}, "m").startswith("unproven"))
        self.assertTrue(DOCK.palette_selected_add_beside({"palette_reads": {"m": [{"supported": True, "snapshot": {"open": False}}]}}, "m").startswith("unproven"))

    def test_b7_add_route_is_header_at_every_width_and_palette_only_when_asked(self) -> None:
        self.assertEqual((DOCK.b7_add_route(False, {}), DOCK.b7_add_route(True, {})), ("more", "more"))
        self.assertEqual((DOCK.b7_add_route(False, {"RAIL_B7_VIA": "palette"}), DOCK.b7_add_route(True, {"RAIL_B7_VIA": "header"})), ("palette", "header"))
        with self.assertRaises(ValueError):
            DOCK.b7_add_route(False, {"RAIL_B7_VIA": "keyboard"})
        import os
        old = os.environ.get("RAIL_B7_VIA")
        os.environ["RAIL_B7_VIA"] = "palette"
        try:
            steps = DOCK.build_b7_steps()  # standard width, palette-driven: its own evidence run
        finally:
            os.environ.pop("RAIL_B7_VIA", None) if old is None else os.environ.__setitem__("RAIL_B7_VIA", old)
        i = next(k for k, x in enumerate(steps) if x.get("mark") == "b7:add-right")
        self.assertEqual(steps[i + 1], {"press": "rail:Commands"})
        self.assertEqual(DOCK.build_b7_steps()[next(k for k, x in enumerate(DOCK.build_b7_steps()) if x.get("mark") == "b7:add-right") + 1], {"press": "@tile[kind=board].more"}, "default is the More route")

    def test_b7_starts_from_two_actual_tiles_by_the_genuine_inputs_and_adds_by_header_at_every_width(self) -> None:
        self.assertEqual(DOCK.seed_two_tiles("rail:Gerbers", "rail:Board", "b7:seed"),
                         [{"press": "rail:Files"}, {"wait": 300}, {"mark": "b7:seed"}, {"press": "rail:Gerbers"}, {"wait": 300}, {"pointer": "rail:Board", "modifier": "shift"}, {"wait": 400}])
        import os
        for size in ("", "minimum"):
            old = os.environ.get("RAIL_WINDOW_SIZE")
            os.environ["RAIL_WINDOW_SIZE"] = size
            try:
                steps = DOCK.build_b7_steps()
            finally:
                os.environ.pop("RAIL_WINDOW_SIZE", None) if old is None else os.environ.__setitem__("RAIL_WINDOW_SIZE", old)
            i = next(k for k, x in enumerate(steps) if x.get("mark") == "b7:seed")
            self.assertEqual(steps[i + 1:i + 4], [{"press": "rail:Gerbers"}, {"wait": 300}, {"pointer": "rail:Board", "modifier": "shift"}], size)
            # the first direction's add is the tile's More button then its Add beside row, never the palette, never a hub action
            i = next(k for k, x in enumerate(steps) if x.get("mark") == "b7:add-right")
            self.assertEqual([x for x in steps[i + 1:i + 6] if "press" in x], [{"press": "@tile[kind=board].more"}, {"press": "@tile[kind=board].more:add-beside"}, {"press": "add-beside:schematic:right"}])
            j = next(k for k, x in enumerate(steps) if x.get("mark") == "b7:pair")
            self.assertEqual(steps[j + 1:j + 4], [{"press": "rail:Schematic"}, {"wait": 300}, {"pointer": "rail:Board", "modifier": "shift"}])
            self.assertFalse([x for x in steps if "action" in x])

    def test_the_more_route_opens_the_tile_menu_for_add_close_maximize_and_restore(self) -> None:
        self.assertEqual([x["press"] for x in DOCK._add_beside("kind=board", "schematic", "left", via="more") if "press" in x],
                         ["@tile[kind=board].more", "@tile[kind=board].more:add-beside", "add-beside:schematic:left"])
        self.assertEqual(DOCK._tile_row("kind=schematic", "close", "more"), [{"press": "@tile[kind=schematic].more"}, {"wait": 120}, {"press": "@tile[kind=schematic].more:close"}])
        self.assertEqual([x.get("press") for x in DOCK._tile_row("kind=schematic", "maximize", "more", row=False) if "press" in x], ["@tile[kind=schematic].more", "@tile[kind=schematic].maximize"])
        self.assertEqual(DOCK._tile_row("kind=schematic", "close", "header"), [{"press": "@tile[kind=schematic].close"}])
        self.assertEqual(DOCK._add_chain("t1", "schematic", "top", "more"), ["tile:t1:more", "tile:t1:more:add-beside", "add-beside:schematic:top"])
        more = [{"id": "tile:t1:more", "found": True}, {"id": "tile:t1:more:add-beside", "found": True}, {"id": "add-beside:schematic:top", "found": True}]
        self.assertEqual(DOCK._add_via({"m": more}, "m"), "more")
        self.assertEqual(DOCK._exact_chain({"m": more}, "m", DOCK._add_chain("t1", "schematic", "top", "more")), more)
        # the wrong tile's More, a skipped row, or an extra press is not the route
        self.assertEqual(DOCK._exact_chain({"m": more}, "m", DOCK._add_chain("t2", "schematic", "top", "more")), [])
        self.assertEqual(DOCK._exact_chain({"m": [more[0], more[2]]}, "m", DOCK._add_chain("t1", "schematic", "top", "more")), [])
        self.assertEqual(DOCK._exact_chain({"m": [*more, more[0]]}, "m", DOCK._add_chain("t1", "schematic", "top", "more")), [])
        # the same steps in the driver at both widths: close, maximize and restore are More rows too
        import os
        for size in ("", "minimum"):
            old = os.environ.get("RAIL_WINDOW_SIZE")
            os.environ["RAIL_WINDOW_SIZE"] = size
            try:
                steps = DOCK.build_b7_steps()
            finally:
                os.environ.pop("RAIL_WINDOW_SIZE", None) if old is None else os.environ.__setitem__("RAIL_WINDOW_SIZE", old)
            # up to the overlay trials (which start from one Board tile and keep their own header add): direction, close, pair, maximize and restore are More rows
            upto = next(k for k, x in enumerate(steps) if x.get("mark") == "b7:overlay-on")
            presses = [x["press"] for x in steps[:upto] if "press" in x]
            for want in ("@tile[kind=schematic].more:close", "@tile[kind=schematic].maximize", "@tile[kind=schematic].restore"):
                self.assertIn(want, presses, size)
            self.assertNotIn("@tile[kind=schematic].close", presses)
            self.assertNotIn("@tile[kind=board].add-beside", presses)

    def test_a_direction_is_scored_from_two_seeded_tiles_plus_one_new_schematic(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            fdir = Path(td)
            lay = {"topbar": {"x": 0, "y": 0, "w": 1440, "h": 36}, "main": {"x": 264, "y": 36, "w": 488, "h": 835}, "rail": {"x": 1369, "y": 36, "w": 71, "h": 835}}
            g, b, sc = ({"x": 753.0, "y": 36.0, "w": 200.0, "h": 835.0}, {"x": 957.0, "y": 36.0, "w": 200.0, "h": 835.0}, {"x": 1161.0, "y": 36.0, "w": 208.0, "h": 835.0})
            g2, b2 = ({"x": 753.0, "y": 36.0, "w": 200.0, "h": 835.0}, {"x": 957.0, "y": 36.0, "w": 412.0, "h": 835.0})
            sp = lambda i, r: {"id": i, "rect": r, "axis": "x"}  # noqa: E731
            start_tiles = [{"id": "tg", "kind": "gerbers", "source": "mfg", "rect": g2, "body": _body_of(g2), "frame": 1}, {"id": "tb", "kind": "board", "source": "b.kicad_pcb", "rect": b2, "body": _body_of(b2), "frame": 1}]
            add_tiles = [{"id": "tg", "kind": "gerbers", "source": "mfg", "rect": g, "body": _body_of(g), "frame": 2},
                         {"id": "tb", "kind": "board", "source": "b.kicad_pcb", "rect": b, "body": _body_of(b), "frame": 2},
                         {"id": "ts", "kind": "schematic", "source": "s.kicad_sch", "rect": sc, "body": _body_of(sc), "frame": 1}]
            _write_painted_png(fdir / "b7-add-right.png", [(g, (60, 180, 120), True), (b, (200, 160, 60), True), (sc, (90, 120, 220), True)])
            shots = {"b7-add-right-start": {"layout": lay, "tiles_seq": 5, "tiles": start_tiles, "splitters": [sp("s1", {"x": 953.0, "y": 36.0, "w": 4.0, "h": 835.0})], "viewer": {}},
                     "b7-add-right": {"layout": lay, "tiles_seq": 15, "tiles": add_tiles,
                                      "splitters": [sp("s1", {"x": 953.0, "y": 36.0, "w": 4.0, "h": 835.0}), sp("s2", {"x": 1157.0, "y": 36.0, "w": 4.0, "h": 835.0})], "viewer": {}}}
            caps = {k: _ok_cap() for k in shots}
            parsed = {"tiles_metric_seen": True, "mark_seqs": {"b7:add-right": 10},
                      "presses": {"b7:add-right": [{"id": "tile:tb:add-beside", "found": True, "seq": 11}, {"id": "add-beside:schematic:right", "found": True, "seq": 12}]}, "shots": shots}
            via_more = json.loads(json.dumps(parsed))
            via_more["presses"]["b7:add-right"] = [{"id": "tile:tb:more", "found": True, "seq": 11}, {"id": "tile:tb:more:add-beside", "found": True, "seq": 12}, {"id": "add-beside:schematic:right", "found": True, "seq": 13}]
            mo = _b7(fdir, via_more, caps, cleared_shots=set(shots))["checks"]["add_beside_directions"]
            self.assertIs(mo["evidence"]["directions"]["right"], True, mo)
            self.assertEqual(mo["evidence"]["add_routes"]["right"], "more")
            ok = _b7(fdir, parsed, caps, cleared_shots=set(shots))["checks"]["add_beside_directions"]
            self.assertEqual(ok["evidence"]["directions"]["right"], True, ok)
            self.assertEqual(ok["evidence"]["add_routes"]["right"], "header")
            # the folded More row is as good as the button; a pointer is not an add
            more = json.loads(json.dumps(parsed))
            more["presses"]["b7:add-right"][0]["id"] = "tile:tb:more:add-beside"
            self.assertIs(_b7(fdir, more, caps, cleared_shots=set(shots))["checks"]["add_beside_directions"]["evidence"]["directions"]["right"], True)
            # the new tile must be a Schematic, and exactly one more than the seed
            wrong = json.loads(json.dumps(parsed))
            wrong["shots"]["b7-add-right"]["tiles"][2]["kind"] = "files"
            self.assertIs(_b7(fdir, wrong, caps, cleared_shots=set(shots))["checks"]["add_beside_directions"]["evidence"]["directions"]["right"], False)
            two = json.loads(json.dumps(parsed))
            two["shots"]["b7-add-right"]["tiles"] = add_tiles[1:]
            self.assertIs(_b7(fdir, two, caps, cleared_shots=set(shots))["checks"]["add_beside_directions"]["evidence"]["directions"]["right"], False)
            # a Schematic on the wrong side of the Board, or a collapsed tile, is not a direction
            left = json.loads(json.dumps(parsed))
            left["shots"]["b7-add-right"]["tiles"][2]["rect"] = {"x": 753.0, "y": 36.0, "w": 100.0, "h": 835.0}
            left["shots"]["b7-add-right"]["tiles"][0]["rect"] = {"x": 1161.0, "y": 36.0, "w": 208.0, "h": 835.0}
            self.assertIsNot(_b7(fdir, left, caps, cleared_shots=set(shots))["checks"]["add_beside_directions"]["evidence"]["directions"]["right"], True)

    def test_an_add_that_cannot_start_because_the_header_has_no_add_beside_is_unavailable_by_design(self) -> None:
        start = {"controls_captured": True, "tiles": [{"id": "t1", "kind": "board"}], "controls_ids": ["rail:Board", "tile:t1:close", "tile:t1:focus"]}
        gone = [{"id": "", "found": False, "seq": 5}]
        why = DOCK.add_beside_absent(start, gone)
        self.assertIn("absent from the Board tile's header", why)
        self.assertIn("tile:t1:add-beside", why)
        # the visible button or its folded row present: a failed press is a different failure, not an absence
        for have in ("tile:t1:add-beside", "tile:t1:more:add-beside"):
            self.assertEqual(DOCK.add_beside_absent({**start, "controls_ids": [*start["controls_ids"], have]}, gone), "", have)
        # nothing measured, nothing attempted, or an ambiguous tile list: no claim of absence
        self.assertEqual(DOCK.add_beside_absent({**start, "controls_captured": False}, gone), "")
        self.assertEqual(DOCK.add_beside_absent(start, [{"id": "tile:t1:add-beside", "found": True}]), "")
        self.assertEqual(DOCK.add_beside_absent({**start, "tiles": []}, gone), "")
        self.assertEqual(DOCK.add_beside_absent(None, gone), "")

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

            inc_layout = {"root": {"type": "split", "id": "s1", "axis": "row", "ratio": 0.5, "a": {"type": "tile", "id": "tb", "kind": "board", "source": "b.kicad_pcb"}, "b": {"type": "tile", "id": "tf", "kind": "files", "source": ""}}}
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
                "refusal_metric_declared": True,
                "mark_seqs": mark_seqs,
                "shots": shots,
                "presses": {
                    **{m: [{"id": "tile:x:" + m, "found": True, "seq": q + 1}] for m, q in mark_seqs.items() if m not in ("b7:resize-drag", "b7:overlay-incompatible")},
                    "b7:overlay-incompatible": [{"id": "tile:tf:overlay-gerbers", "found": True, "seq": 121}],
                },
                "drags": {"b7:resize-drag": [{"ok": True, "splitter": "sp-x", "from": [1061, 420], "to": [975, 420], "moves": 6, "seq": 71}]},
                "refusals": {"b7:overlay-incompatible": [{"code": "incompatibleOverlay", "text": "Those views cannot be overlaid", "seq": 122}]},
            }

            b7 = _b7(fdir, parsed, caps, cleared_shots=cleared)
            self.assertEqual(b7["status"], "measured", b7)
            self.assertEqual(b7["clean"], 7)

            # the minimum window initiates the adds through the command palette: `rail:Commands`, "add be…" highlighted `add-beside` before Enter, then the page entry
            p_pal = json.loads(json.dumps(parsed))
            for side in ("right", "left", "bottom", "top"):
                mark = f"b7:add-{side}"
                q = mark_seqs[mark]
                p_pal["presses"][mark] = [{"id": "rail:Commands", "found": True, "seq": q + 1}, {"id": f"add-beside:schematic:{side}", "found": True, "seq": q + 3}]
                p_pal.setdefault("palette_reads", {})[mark] = [{"supported": True, "seq": q + 2, "snapshot": {"open": True, "selected_id": "add-beside", "label": "Add beside…"}}]
            b7_pal = _b7(fdir, p_pal, caps, cleared_shots=cleared)
            self.assertEqual(b7_pal["checks"]["add_beside_directions"]["status"], "measured", b7_pal["checks"]["add_beside_directions"])
            self.assertTrue(b7_pal["checks"]["add_beside_directions"]["passed"])
            # the wrong highlighted item is a failure; no read at all is unavailable (unproven), never a pass
            p_wrong = json.loads(json.dumps(p_pal))
            p_wrong["palette_reads"]["b7:add-right"][0]["snapshot"]["selected_id"] = "rail:board"
            wrong = _b7(fdir, p_wrong, caps, cleared_shots=cleared)["checks"]["add_beside_directions"]
            self.assertEqual((wrong["status"], wrong["passed"]), ("measured", False))
            self.assertIn("not 'add-beside'", wrong["note"])
            p_noread = json.loads(json.dumps(p_pal))
            p_noread["palette_reads"]["b7:add-left"] = []
            noread = _b7(fdir, p_noread, caps, cleared_shots=cleared)["checks"]["add_beside_directions"]
            self.assertIn(noread["status"], ("unavailable", "measured"))
            self.assertIsNot(noread.get("passed"), True)
            self.assertIn("unproven", noread["note"])

            # Missing pane geometry on close & maximize -> unavailable (never waived!)
            p_nogeom = json.loads(json.dumps(parsed))
            p_nogeom["shots"]["b7-close-right"]["layout"] = {}
            p_nogeom["shots"]["b7-maximized"]["layout"] = {}
            b7_ng = _b7(fdir, p_nogeom, caps, cleared_shots=cleared)
            self.assertEqual(b7_ng["checks"]["close"]["status"], "unavailable")
            self.assertEqual(b7_ng["checks"]["maximize_restore"]["status"], "unavailable")

            # String "false" in drag.ok -> resize_drag unavailable
            p_strfalse = json.loads(json.dumps(parsed))
            p_strfalse["drags"]["b7:resize-drag"] = [{"ok": "false", "splitter": "sp-x", "from": [1061, 420], "to": [975, 420]}]
            b7_sf = _b7(fdir, p_strfalse, caps, cleared_shots=cleared)
            self.assertEqual(b7_sf["checks"]["resize_drag"]["status"], "unavailable")

            # Stale inventory: tiles drawn BEFORE the delivered press (carried forward) -> unavailable, per transition
            p_stale = json.loads(json.dumps(parsed))
            p_stale["shots"]["b7-after-swap"]["tiles_seq"] = 60  # the swap press was delivered at seq 61
            b7_st = _b7(fdir, p_stale, caps, cleared_shots=cleared)
            self.assertEqual(b7_st["checks"]["swap"]["status"], "unavailable")
            self.assertIn("stale", b7_st["checks"]["swap"]["note"])

            # A transition whose press was never delivered is not fresh either (the mark alone is not enough)
            p_nopress = json.loads(json.dumps(parsed))
            p_nopress["presses"]["b7:maximize"][0]["found"] = False
            self.assertEqual(_b7(fdir, p_nopress, caps, cleared_shots=cleared)["checks"]["maximize_restore"]["status"], "unavailable")

            # Drag naming a splitter that is not in the drawn inventory, or moving across its axis, fails (measured)
            p_wrong = json.loads(json.dumps(parsed))
            p_wrong["drags"]["b7:resize-drag"][0]["splitter"] = "ghost"
            self.assertIs(_b7(fdir, p_wrong, caps, cleared_shots=cleared)["checks"]["resize_drag"]["passed"], False)
            p_across = json.loads(json.dumps(parsed))
            p_across["drags"]["b7:resize-drag"][0]["to"] = [1061, 300]
            self.assertIs(_b7(fdir, p_across, caps, cleared_shots=cleared)["checks"]["resize_drag"]["passed"], False)

            # Refusal: hub layout not observed -> unavailable; raw code in the human text -> failed
            def _ref(p):
                return _b7(fdir, p, caps, cleared_shots=cleared)["checks"]["incompatible_overlay_refusal"]
            p_nolay = json.loads(json.dumps(parsed))
            del p_nolay["shots"]["b7-overlay-incompat"]["viewer"]["layout"]
            self.assertEqual(_ref(p_nolay)["status"], "unavailable")
            # two empty dicts are not "unchanged"
            p_empty = json.loads(json.dumps(parsed))
            p_empty["shots"]["b7-overlay-incompat"]["viewer"]["layout"] = {}
            p_empty["shots"]["b7-overlay-incompat-before"]["viewer"]["layout"] = {}
            self.assertEqual(_ref(p_empty)["status"], "unavailable")
            # starting tree must be Board + Files
            p_tree = json.loads(json.dumps(parsed))
            for k in ("b7-overlay-incompat", "b7-overlay-incompat-before"):
                p_tree["shots"][k]["viewer"]["layout"]["root"]["b"]["kind"] = "schematic"
            self.assertEqual(_ref(p_tree)["status"], "unavailable")
            # a refusal reported BEFORE the delivered press does not count: with refusals declared observable -> failed;
            # with no such declaration in the run -> unavailable (nothing proves a refusal could be seen)
            p_old = json.loads(json.dumps(parsed))
            p_old["refusals"]["b7:overlay-incompatible"][0]["seq"] = 100
            r_old = _ref(p_old)
            self.assertEqual(r_old["status"], "measured")
            self.assertIs(r_old["passed"], False)
            p_undeclared = json.loads(json.dumps(p_old))
            p_undeclared["refusal_metric_declared"] = False
            self.assertEqual(_ref(p_undeclared)["status"], "unavailable")
            p_none = json.loads(json.dumps(parsed))
            p_none["refusals"] = {}
            self.assertIs(_ref(p_none)["passed"], False)
            p_none["refusal_metric_declared"] = False
            self.assertEqual(_ref(p_none)["status"], "unavailable")
            # an event that exists but is wrong still fails without any declaration
            p_wrong = json.loads(json.dumps(parsed))
            p_wrong["refusal_metric_declared"] = False
            p_wrong["refusals"]["b7:overlay-incompatible"][0]["code"] = "locked"
            self.assertIs(_ref(p_wrong)["passed"], False)
            # press on some other control / undelivered -> unavailable
            p_other = json.loads(json.dumps(parsed))
            p_other["presses"]["b7:overlay-incompatible"][0]["id"] = "tile:tb:overlay-gerbers"
            self.assertEqual(_ref(p_other)["status"], "unavailable")
            # layout changed by the refused press -> failed
            p_chg = json.loads(json.dumps(parsed))
            p_chg["shots"]["b7-overlay-incompat"]["viewer"]["layout"]["root"]["ratio"] = 0.3
            self.assertIs(_ref(p_chg)["passed"], False)
            p_raw = json.loads(json.dumps(parsed))
            p_raw["refusals"]["b7:overlay-incompatible"][0]["text"] = "incompatibleOverlay"
            self.assertIs(_b7(fdir, p_raw, caps, cleared_shots=cleared)["checks"]["incompatible_overlay_refusal"]["passed"], False)

            # Identical bodies in pixels (two tiles painted alike) -> add_beside_directions fails
            _write_painted_png(fdir / "b7-add-right.png", [(r_l, c_brd, True), (r_r, c_brd, True)])
            b7_id = _b7(fdir, parsed, caps, cleared_shots=cleared)
            self.assertIs(b7_id["checks"]["add_beside_directions"]["passed"], False)

            # Per-shot privacy clearance is consulted
            self.assertEqual(_b7(fdir, parsed, caps, cleared_shots=set())["checks"]["swap"]["status"], "unavailable")

            # Scored over all seven: one unmeasured check is not earned
            self.assertEqual(b7_ng["total"], 7)

    def test_b7_steps_collapse_two_tile_layout_before_idle_resets(self) -> None:
        steps = DOCK.build_b7_steps()
        pair_idle = next(i for i, s in enumerate(steps) if isinstance(s, dict) and s.get("tag") == "b7-pair-idle")
        overlay_idle = next(i for i, s in enumerate(steps) if isinstance(s, dict) and s.get("tag") == "b7-overlay-idle")
        b7_restore = next(i for i, s in enumerate(steps) if isinstance(s, dict) and s.get("mark") == "b7:restore")
        b7_close_top = next(i for i, s in enumerate(steps) if isinstance(s, dict) and s.get("mark") == "b7:close-top")
        self.assertIn({"press": "@tile[kind=gerbers].more:close"}, steps[b7_close_top:pair_idle])
        self.assertIn({"press": "@tile[kind=schematic].more:close"}, steps[b7_restore:overlay_idle])



if __name__ == "__main__":
    unittest.main()
