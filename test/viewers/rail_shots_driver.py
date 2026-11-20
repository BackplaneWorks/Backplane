#!/usr/bin/env python3
"""Script generator and external screenshot/obstruction capture daemon for rail-shots.sh."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_dock as DOCK
import rail_auto as AUTO
import rail_shots_manifest as M  # noqa: E402

# R1's position (mm) in the full fixture's board (test/files/fixtures/plot/ecc83-pp.kicad_pcb: `(at 136.271 107.95 -90)`):
# the client maps it through the drawn view, so the tap lands on R1 at any pane size or text scale
R1_AT_MM = (136.271, 107.95)
SHOT_NAME_RE = re.compile(r"^[a-z0-9-]+$")


def build_script(kind: str, early_ms: int = 300, settle_timeout_ms: int = 15000, embed: bool = False) -> list[dict]:
    steps: list[dict] = [
        {"select": 0},
        {"shot": "idle", "settle": "", "timeout_ms": settle_timeout_ms},
    ]

    for slug, rail_id, settle_kind, _ in M.PAGES:
        if slug == "idle":
            continue
        assert rail_id is not None
        # Shorter settle timeout for pages whose resource is absent on this project kind
        # (so e.g. mech-only/split-after-mech doesn't stall for 15s when 3D stays stuck loading).
        tmo = settle_timeout_ms if slug in M.HAS_RESOURCE.get(kind, set()) else min(settle_timeout_ms, 8000)
        steps.extend([
            {"mark": f"press:{slug}"},
            {"press": rail_id},
            {"wait": early_ms},
            {"mark": f"early:{slug}"},
            {"shot": f"{slug}-300ms"},
            {"mark": f"settled:{slug}"},
            {"shot": slug, "settle": settle_kind, "timeout_ms": tmo},
        ])

        if kind == "full":
            if slug == "gerbers":
                # Task 4: press "vs board" on Gerbers and capture settled state
                steps.extend([
                    {"mark": "press:task4-gerbers-vs-board"},
                    {"press": "ui:vs board"},
                    {"wait": 600},
                    {"mark": "settled:task4-gerbers-vs-board"},
                    {"shot": "task4-gerbers-vs-board", "settle": "gerbers", "timeout_ms": settle_timeout_ms},
                ])
            elif slug == "split":
                # Task 5: tap known component R1 on the board pane (slot 2) in Split and check cross-probe
                steps.extend([
                    {"mark": "press:task5-split-probe"},
                    {"tap_mm": [R1_AT_MM[0], R1_AT_MM[1], 2]},
                    {"wait": 600},
                    {"mark": "settled:task5-split-probe"},
                    {"shot": "task5-split-probe", "settle": "split", "timeout_ms": settle_timeout_ms},
                    # Task 5 one step back: press lit rail:Split (closes viewer or returns to previous page)
                    {"mark": "press:task5-back"},
                    {"press": "rail:Split"},
                    {"mark": "settled:task5-back"},
                    {"shot": "task5-back", "settle": "", "timeout_ms": min(settle_timeout_ms, 5000)},
                ])
            elif slug == "board":
                # Task 9: press Follow agent on Board and verify viewer.follow.on + visible state, then one step back
                steps.extend([
                    {"mark": "press:task9-follow-on"},
                    {"press": "follow-agent"},
                    {"wait": 600},
                    {"mark": "settled:task9-follow-on"},
                    {"shot": "task9-follow-on", "settle": "board", "timeout_ms": settle_timeout_ms},
                    {"mark": "press:task9-follow-off"},
                    {"press": "follow-agent"},
                    {"wait": 600},
                    {"mark": "settled:task9-follow-off"},
                    {"shot": "task9-follow-off", "settle": "board", "timeout_ms": settle_timeout_ms},
                    # Task 1 one step back: press lit rail:Board to shut viewer and restore idle width
                    {"mark": "press:task1-back"},
                    {"press": "rail:Board"},
                    {"mark": "settled:task1-back"},
                    {"shot": "task1-back", "settle": "", "timeout_ms": settle_timeout_ms},
                ])
            elif slug == "3d":
                # Task 2 one step back: press lit rail:3D to shut viewer and restore idle width
                steps.extend([
                    {"mark": "press:task2-back"},
                    {"press": "rail:3D"},
                    {"mark": "settled:task2-back"},
                    {"shot": "task2-back", "settle": "", "timeout_ms": settle_timeout_ms},
                ])

    if kind == "full":
        # Task 6 one step back (kept separate so mech -> split-after-mech stays a direct transition)
        steps.extend([
            {"mark": "task6-back-setup"},
            {"press": "rail:Mech"},
            {"wait_settled": "mech", "tag": "task6-back-start", "timeout_ms": settle_timeout_ms},
            {"mark": "press:task6-back"},
            {"press": "rail:Mech"},
            {"mark": "settled:task6-back"},
            {"shot": "task6-back", "settle": "", "timeout_ms": settle_timeout_ms},
        ])
        # Task 3: verify 1-press return to Schematic from every other page (each starting page settled first)
        for src_slug, src_rail, src_settle in M.TASK3_SOURCES:
            steps.extend([
                {"mark": f"task3-setup:{src_slug}"},
                {"press": src_rail},
                {"wait_settled": src_settle, "tag": f"task3-start-{src_slug}", "timeout_ms": settle_timeout_ms},
                {"mark": f"press:task3-from-{src_slug}"},
                {"press": "rail:Schematic"},
                {"mark": f"settled:task3-from-{src_slug}"},
                {"shot": f"task3-from-{src_slug}", "settle": "schematic", "timeout_ms": settle_timeout_ms},
            ])
        # Task 10: close the viewer from an open page (Schematic -> press rail:Schematic) and verify idle width
        # (Task 5's R1 probe is still active in pv.lit when Schematic closes.)
        steps.extend([
            {"mark": "press:task10-closed"},
            {"press": "rail:Schematic"},
            {"mark": "settled:task10-closed"},
            {"shot": "task10-closed", "settle": "", "timeout_ms": settle_timeout_ms},
            # Extra check (D): swallowed-Escape regression check — with R1 highlighted and viewer closed,
            # focus composer, send ONE Escape, then bare rail key 'b' and record whether Board opens.
            {"keys": "ctrl-l"},
            {"wait": 60},
            {"mark": "extra:escape-after-closed-probe"},
            {"keys": "escape"},
            {"wait": 60},
            {"keys": "b"},
            {"wait_settled": "board", "tag": "extra-escape-after-closed-probe", "timeout_ms": min(settle_timeout_ms, 2500)},
            # Now deterministically clear the Task 5 R1 probe while Schematic is open (works whether Board opened or stayed closed)
            # and return to idle before B1 probes.
            {"press": "rail:Schematic"},
            {"wait_settled": "schematic", "tag": "b1-probe-clear-open", "timeout_ms": min(settle_timeout_ms, 4000)},
            {"action": ["view-probe", ""]},
            {"wait": 120},
            {"press": "rail:Schematic"},
            {"wait_settled": "", "tag": "b1-probe-clear-close", "timeout_ms": min(settle_timeout_ms, 3000)},
            {"keys": "ctrl-l"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 60},
        ])
    else:
        # Close the last opened page (Browser) so Rule B1 probes start from idle.
        steps.extend([
            {"press": "rail:Browser"},
            {"wait_settled": "", "tag": "b1-start-idle", "timeout_ms": min(settle_timeout_ms, 4000)},
        ])

    # Pointer-route calibration: a real mouse press at an ENABLED row's rect must open it, so a later pointer press that does
    # nothing on a disabled row is a measured no-op and not a click that missed.
    steps.extend([
        {"mark": "b1:pointer-calibration"},
        {"pointer": "rail:Files"},
        {"wait_settled": "files", "tag": "b1-pointer-calibration", "timeout_ms": min(settle_timeout_ms, 4000)},
        {"keys": "ctrl-l"},
        {"wait": 80},
        {"keys": "escape"},
        {"wait": 80},
        {"keys": "escape"},
        {"wait": 80},
        {"wait_settled": "", "tag": "b1-pointer-calibration-idle", "timeout_ms": min(settle_timeout_ms, 3000)},
    ])

    # Rule B1: one selection behaviour for every rail entry (press open/close, bare key open/close, palette open, disabled no-op).
    avail = M.HAS_RESOURCE.get(kind, set())
    for eid, _label, rail_id, settle_kind, bare_key, pal_query in M.B1_ENTRIES:
        pal_type = " ".join(pal_query.lower())
        if eid in avail:
            steps.extend([
                {"mark": f"b1:press-open:{eid}"},
                {"press": rail_id},
                {"wait_settled": settle_kind, "tag": f"b1-press-open-{eid}", "timeout_ms": min(settle_timeout_ms, 6000)},
                {"mark": f"b1:press-close:{eid}"},
                {"press": rail_id},
                {"wait_settled": "", "tag": f"b1-press-close-{eid}", "timeout_ms": min(settle_timeout_ms, 3000)},
                # Focus root_focus deterministically (ctrl-l focuses composer; escape moves to root_focus)
                {"keys": "ctrl-l"},
                {"wait": 60},
                {"keys": "escape"},
                {"wait": 60},
                {"mark": f"b1:key-open:{eid}"},
                {"keys": bare_key},
                {"wait_settled": settle_kind, "tag": f"b1-key-open-{eid}", "timeout_ms": min(settle_timeout_ms, 2500)},
                {"mark": f"b1:key-close:{eid}"},
                {"keys": bare_key},
                {"wait_settled": "", "tag": f"b1-key-close-{eid}", "timeout_ms": min(settle_timeout_ms, 2500)},
                # Ensure viewer is closed before palette test (ctrl-l + escape closes if still open)
                {"keys": "ctrl-l"},
                {"wait": 50},
                {"keys": "escape"},
                {"wait": 80},
                {"mark": f"b1:palette-open:{eid}"},
                {"press": "rail:Commands"},
                {"wait": 120},
                {"keys": pal_type},
                {"wait": 120},
                {"read": "palette"},
                {"keys": "enter"},
                {"wait_settled": settle_kind, "tag": f"b1-palette-open-{eid}", "timeout_ms": min(settle_timeout_ms, 4000)},
                # Close viewer (or palette if it didn't match) without ever toggling a closed viewer open;
                # send two escapes so a first escape that clears a palette/probe/composer still closes any open viewer.
                {"keys": "ctrl-l"},
                {"wait": 80},
                {"keys": "escape"},
                {"wait": 80},
                {"keys": "escape"},
                {"wait": 80},
                {"wait_settled": "", "tag": f"b1-cleanup-{eid}", "timeout_ms": min(settle_timeout_ms, 3000)},
            ])
        else:
            steps.extend([
                {"mark": f"b1:disabled-press:{eid}"},
                {"pointer": rail_id},
                {"wait": 150},
                {"read": "rail"},
                {"wait": 60},
                {"wait_settled": "", "tag": f"b1-disabled-press-{eid}", "timeout_ms": 1200},
                {"keys": "ctrl-l"},
                {"wait": 60},
                {"keys": "escape"},
                {"wait": 60},
                {"mark": f"b1:disabled-key:{eid}"},
                {"keys": bare_key},
                {"wait": 150},
                {"read": "rail"},
                {"wait": 60},
                {"wait_settled": "", "tag": f"b1-disabled-key-{eid}", "timeout_ms": 1200},
                {"mark": f"b1:disabled-palette:{eid}"},
                {"press": "rail:Commands"},
                {"wait": 120},
                {"keys": pal_type},
                {"wait": 120},
                {"read": "palette"},
                # Close palette without pressing Enter so another matching command (e.g. shortcuts) is never run
                {"keys": "escape"},
                {"wait": 150},
                {"read": "rail"},
                {"wait": 60},
                {"wait_settled": "", "tag": f"b1-disabled-palette-{eid}", "timeout_ms": 1200},
            ])

    # Rule B1 tool entries: Terminal (toggles its panel) and Commands (opens the palette)
    steps.extend([
        {"mark": "b1:terminal-press-open"},
        {"press": "rail:Terminal"},
        {"wait": 200},
        {"read": "rail"},
        {"wait": 60},
        {"wait_settled": "", "tag": "b1-terminal-press-open", "timeout_ms": 1500},
        {"mark": "b1:terminal-press-close"},
        {"press": "rail:Terminal"},
        {"wait": 200},
        {"read": "rail"},
        {"wait": 60},
        {"wait_settled": "", "tag": "b1-terminal-press-close", "timeout_ms": 1500},
        {"keys": "ctrl-l"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 60},
        {"mark": "b1:terminal-key-open"},
        {"keys": "`"},
        {"wait": 200},
        {"read": "rail"},
        {"wait": 60},
        {"wait_settled": "", "tag": "b1-terminal-key-open", "timeout_ms": 1500},
        {"mark": "b1:terminal-key-close"},
        {"keys": "shift-escape"},
        {"wait": 200},
        {"read": "rail"},
        {"wait": 60},
        {"wait_settled": "", "tag": "b1-terminal-key-close", "timeout_ms": 1500},
        {"mark": "b1:commands-press-open"},
        {"press": "rail:Commands"},
        {"wait": 150},
        {"read": "palette"},
        {"mark": "b1:commands-escape-close"},
        {"keys": "escape"},
        {"wait": 120},
        {"read": "palette"},
    ])

    # Extra missing-document checks run BEFORE B1 Simulation (under --embed) so each starts from a known clean state
    # without a lingering SIMULATOR frame from B1 Simulation; capture hub session inventory AT the idle reset.
    if kind in M.EXTRA_MISSING_SHOTS:
        steps.extend([
            {"keys": "ctrl-l"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 60},
            {"mark": "extra:extra-missing-start-idle"},
            {"shot": "extra-missing-start-idle", "settle": "", "timeout_ms": min(settle_timeout_ms, 3000)},
        ])
        missing_targets = [("board", "extra-missing-board"), ("3d", "extra-missing-3d")] if kind == "schematic-only" else [("schematic", "extra-missing-schematic")]
        for view_kind, shot_name in missing_targets:
            steps.extend([
                {"mark": f"extra:{shot_name}"},
                {"viewshow": [view_kind, ""]},
                {"wait": 800},
                {"shot": shot_name, "settle": view_kind, "timeout_ms": min(settle_timeout_ms, 8000)},
            ])
        steps.extend([
            {"keys": "ctrl-l"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 60},
            {"wait_settled": "", "tag": "extra-missing-end-idle", "timeout_ms": min(settle_timeout_ms, 3000)},
        ])

    # Rule B1 under --embed: Simulation (available when a schematic is present)
    if embed and "schematic" in avail:
        steps.extend([
            {"mark": "b1:sim-press-open"},
            {"press": "rail:Simulation"},
            {"wait": 600},
            {"kicad": {"report": True, "tag": "b1-sim-open"}},
            {"read": "rail"},
            {"wait": 80},
            {"wait_settled": "schematic", "tag": "b1-sim-open", "timeout_ms": min(settle_timeout_ms, 4000)},
            {"mark": "b1:sim-cleanup"},
            {"click": "ui:View"},
            {"wait": 300},
            {"keys": "ctrl-l"},
            {"wait": 60},
            {"keys": "escape"},
            {"wait": 80},
            {"keys": "escape"},
            {"wait": 100},
            {"kicad": {"report": True, "tag": "b1-sim-cleanup-report"}},
            {"shot": "b1-sim-cleanup", "settle": "", "timeout_ms": min(settle_timeout_ms, 3000)},
        ])

    if kind == "full" and embed:
        # Task 8: move between Backplane's viewer and embedded KiCad editor (Board -> Edit -> View)
        steps.extend([
            {"mark": "task8-setup"},
            {"press": "rail:Board"},
            {"wait_settled": "board", "tag": "task8-start-board", "timeout_ms": settle_timeout_ms},
            {"mark": "press:task8-edit"},
            {"click": "ui:Edit", "within": 8000},
            {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "task8-edit"}},
            {"wait": 500},
            {"mark": "settled:task8-edit"},
            {"shot": "task8-edit"},
            {"mark": "press:task8-view"},
            {"click": "ui:View", "within": 8000},
            {"wait": 800},
            {"kicad": {"report": True, "tag": "task8-view"}},
            {"mark": "settled:task8-view"},
            {"shot": "task8-view", "settle": "board", "timeout_ms": settle_timeout_ms},
            # Task 7: run a simulation of the schematic (rail:Simulation -> Simulator -> Run)
            {"mark": "press:task7-sim"},
            {"click": "rail:Simulation", "within": 8000},
            {"kicad": {"wait_painted": 60000, "kind": "SIMULATOR", "tag": "task7-sim"}},
            {"wait": 800},
            {"mark": "settled:task7-sim"},
            {"shot": "task7-sim"},
            {"mark": "embed:sim-run"},
            {"wait": 3000},
            {"kicad": {"report": True, "tag": "task7-sim-run"}},
            {"mark": "settled:task7-sim-run"},
            {"shot": "task7-sim-run"},
        ])

    if kind in M.EXTRA_MISSING_SHOTS and embed:
        # Scripted Edit from Files/Review/Browser via test-mode `script:kicad-edit` (PR #1353 route: navigates to design page, then edits).
        expected_frame = "SCH" if kind == "schematic-only" else "PCB"
        for slug, rail_id, settle_kind in M.EXTRA_EDIT_PAGES:
            shot_name = f"extra-edit-{slug}"
            steps.extend([
                {"mark": f"extra:{shot_name}"},
                {"press": rail_id},
                {"wait_settled": settle_kind, "tag": f"{shot_name}-start", "timeout_ms": settle_timeout_ms},
                {"mark": f"press:{shot_name}"},
                {"click": "script:kicad-edit", "within": 4000},
                {"kicad": {"wait_painted": 20000, "kind": expected_frame, "tag": shot_name}},
                {"wait": 600},
                {"kicad": {"report": True, "tag": f"{shot_name}-report"}},
                {"shot": shot_name},
                {"mark": f"cleanup:{shot_name}"},
                {"click": "ui:View", "within": 4000},
                {"wait": 500},
            ])
    elif kind in M.NO_DESIGN_KINDS and embed:
        # Scripted Edit on a project with no KiCad document: must start no KiCad and report Files status.
        steps.extend([
            {"mark": "extra:extra-edit-files"},
            {"press": "rail:Files"},
            {"wait_settled": "files", "tag": "extra-edit-files-start", "timeout_ms": settle_timeout_ms},
            {"mark": "press:extra-edit-files"},
            {"click": "script:kicad-edit", "within": 1500},
            {"wait": 800},
            {"kicad": {"report": True, "tag": "extra-edit-files-report"}},
            {"shot": "extra-edit-files"},
        ])

    if kind == "full":
        # Docking/tiling (Task 4 user route, Rule B7): scheduled in the normal `full` run, UI controls and drawn-inventory selectors only;
        # every check stays `unavailable` until a build emits `GPUI_METRIC event=tiles` (docs: rail_dock.py contract table).
        steps.extend(DOCK.build_task4_dock_steps(settle_timeout_ms))
        steps.extend(DOCK.build_b7_steps(settle_timeout_ms))

    if kind == "full" and embed:
        # Decision-5 gates (auto-attach): AFTER every existing task and check, so earlier measurements are untouched; see rail_auto.py
        steps.extend(AUTO.build_auto_steps(settle_timeout_ms))

    return steps


def detect_obstructions(disp: str) -> tuple[list[dict], bool, str | None]:
    """Inspect X11 top-level windows on `disp` for native dialogs/windows obstructing Backplane.

    Returns (obstructions, inspect_ok, inspect_error). Fails closed (inspect_ok=False) if xwininfo
    cannot inspect the X11 tree or a candidate window. Children nested inside Backplane's own
    window (e.g. embedded KiCad or CEF) are ignored.
    """
    env = dict(os.environ, DISPLAY=disp)
    try:
        out = subprocess.check_output(["xwininfo", "-root", "-tree"], env=env, text=True, stderr=subprocess.DEVNULL, timeout=5)
    except Exception as e:
        return [], False, f"xwininfo -root -tree failed: {e}"
    obstructions: list[dict] = []
    inspect_ok = True
    inspect_error: str | None = None
    # Match lines like:
    #   0x600003 "Warning": ("kicad" "Kicad")  610x120+415+390  +415+390
    #   0x400001 "Backplane": ()  1440x900+0+0  +0+0
    #   0x600003 (has no name): ("kicad" "Kicad")  610x120+415+390  +415+390
    win_re = re.compile(r'^(\s*)(0x[0-9a-f]+)\s+(?:"([^"]*)"|\(has no name\)):\s+(?:\("([^"]*)"\s+"([^"]*)"\)|\(\))\s+(\d+)x(\d+)\+(-?\d+)\+(-?\d+)')
    bp_indent: int | None = None
    for line in out.splitlines():
        m = win_re.match(line)
        if not m:
            continue
        indent_s, xid, title, cls1, cls2, w_s, h_s, x_s, y_s = m.groups()
        indent = len(indent_s)
        title = title or ""
        cls1 = cls1 or ""
        cls2 = cls2 or ""
        w, h = int(w_s), int(h_s)
        low_cls = f"{cls1} {cls2}".lower()
        low_title = title.lower()
        if "backplane" in low_cls or "backplane" in low_title:
            bp_indent = indent
            continue
        if bp_indent is not None:
            if indent > bp_indent:
                # Embedded child window inside Backplane (e.g. KiCad container/frame or CEF).
                continue
            bp_indent = None
        if w < 60 or h < 40:
            continue
        # Ignore i3 WM frames
        if "i3" in low_cls:
            continue
        # Confirm it is actually mapped (IsViewable); fail closed if inspection fails
        try:
            info = subprocess.check_output(["xwininfo", "-id", xid], env=env, text=True, stderr=subprocess.DEVNULL, timeout=2)
            if "Map State: IsViewable" not in info:
                continue
        except Exception as e:
            inspect_ok = False
            inspect_error = f"xwininfo -id {xid} failed: {e}"
            continue
        obstructions.append({
            "xid": xid,
            "title": title,
            "class": [cls1, cls2],
            "w": w,
            "h": h,
            "x": int(x_s),
            "y": int(y_s),
        })
    return obstructions, inspect_ok, inspect_error


def trigger_sim_run(disp: str, last_kicad: dict, kind_out: Path) -> None:
    """Click inside the embedded Simulator frame and press `r` (Run/Stop Simulation).

    Records observer metadata explicitly: until KiCad IPC or the client exposes completed
    simulation results (trace / operating-point values), `observer` is None and
    `result_evidenced` is False.
    """
    repo_root = Path(__file__).resolve().parents[2]
    xinput = repo_root / "test" / "cef" / "xinput.py"
    env = dict(os.environ, DISPLAY=disp)
    pane = last_kicad.get("pane") or last_kicad.get("container") or {}
    x = int(pane.get("x", 800)) + max(40, int(pane.get("w", 400)) // 5)
    y = int(pane.get("y", 100)) + max(40, int(pane.get("h", 400)) * 3 // 4)
    rec: dict = {
        "action": "sim-run",
        "x": x,
        "y": y,
        "key": "r",
        "ok": False,
        "observer": None,
        "result_evidenced": False,
        "result_summary": "",
        "error": None,
    }
    try:
        if not xinput.exists():
            raise FileNotFoundError(str(xinput))
        subprocess.run([sys.executable, str(xinput), "click", str(x), str(y)], env=env, check=True, timeout=10)
        time.sleep(0.35)
        subprocess.run([sys.executable, str(xinput), "key", "r"], env=env, check=True, timeout=10)
        rec["ok"] = True
    except Exception as e:
        rec["error"] = str(e)
    with (kind_out / "embed_actions.jsonl").open("a", encoding="utf-8") as af:
        af.write(json.dumps(rec, separators=(",", ":")) + "\n")


def _parse_hub_kicad_sessions_output(out: str) -> dict:
    """Parse stdout of `hub_rpc.mjs` after a `hub.status` probe on `/ws?view=1`.

    On `/ws?view=1`, the hub flushes `view_info_frame` (every `editor.kicad.<digest>` entry)
    before servicing subsequent RPCs, so a confirmed `hub.status` reply (`ok: true`, dict `result`)
    with zero `INFO editor.kicad.*` lines is a genuinely empty session inventory, distinct from
    an unconfirmed or broken read (`inventory_complete: False`).
    """
    rpc_ok = False
    sessions: dict[str, dict] = {}
    for line in out.splitlines():
        if line.startswith("INFO "):
            try:
                obj = json.loads(line[5:])
            except Exception as e:
                return {"ok": False, "inventory_complete": False, "error": f"malformed INFO json: {e}", "sessions": {}}
            key = obj.get("key") or ""
            if not key.startswith("editor.kicad."):
                continue
            val = obj.get("value")
            if not isinstance(val, dict):
                return {"ok": False, "inventory_complete": False, "error": f"non-object session value for {key}", "sessions": {}}
            raw_rt = val.get("runtime")
            frames = [
                {"kind": f.get("kind"), "shown": bool(f.get("shown"))}
                for f in (val.get("frames") or [])
                if isinstance(f, dict)
            ]
            rt = raw_rt if isinstance(raw_rt, dict) else {}
            entry = {
                "state": val.get("state"),
                "active": val.get("active"),
                "pid": val.get("pid"),
                "generation": rt.get("generation", val.get("generation")),
                "frames": frames,
            }
            # Field presence is evidence: a session that sent no `runtime`, or a `runtime` with no `holder`, must not
            # arrive as `holder: null` (absent holder). Only keys the hub really sent are carried.
            if isinstance(raw_rt, dict):
                entry["runtime"] = {k: raw_rt[k] for k in ("holder", "session", "generation") if k in raw_rt}
            sessions[key] = entry
        elif line.startswith("{"):
            try:
                rep = json.loads(line)
                if rep.get("m") == "hub.status" and rep.get("ok") is True and isinstance(rep.get("result"), dict):
                    rpc_ok = True
            except Exception:
                pass
    if not rpc_ok:
        return {"ok": False, "inventory_complete": False, "error": "hub_rpc did not confirm hub.status reply on /ws?view=1", "sessions": {}}
    return {"ok": True, "inventory_complete": True, "sessions": sessions}


def query_hub_kicad_sessions(port: str, token_file: str, bun_bin: str) -> dict:
    """Query the hub's authoritative `editor.kicad.*` session inventory via `hub_rpc.mjs` (never /proc).

    Sends `hub.status` (a `/ws?view=1` view-transport RPC) so we only mark `inventory_complete: True`
    after the hub's initial `view_info_frame` has been flushed and acknowledged; any RPC error or
    malformed `editor.kicad.*` INFO line fails closed (`ok: False`).
    """
    if not port or not token_file or not bun_bin:
        return {"ok": False, "inventory_complete": False, "error": "hub_rpc env missing", "sessions": {}}
    helper = Path(__file__).resolve().parents[1] / "kicad_embed" / "lib" / "hub_rpc.mjs"
    if not helper.exists():
        return {"ok": False, "inventory_complete": False, "error": f"hub_rpc helper missing: {helper}", "sessions": {}}
    env = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY")}
    try:
        out = subprocess.check_output(
            [bun_bin, str(helper), str(port), token_file, '[{"m":"hub.status","p":{}}]'],
            env=env, text=True, stderr=subprocess.PIPE, timeout=10,
        )
    except Exception as e:
        return {"ok": False, "inventory_complete": False, "error": str(e), "sessions": {}}
    return _parse_hub_kicad_sessions_output(out)


COMPOSER_POINTER = (440, 790)  # inside the composer of the 1440x900 test window (left of the viewer pane)
_FIRST_GENERATION: list[int] = []  # the running KiCad's generation seen by the first stale probe (the "old" one after Edit restarts it)


def _single_session(port: str, token: str, bun: str) -> dict | None:
    inv = query_hub_kicad_sessions(port, token, bun)
    sessions = list((inv.get("sessions") or {}).values()) if inv.get("ok") and inv.get("inventory_complete") else []
    return sessions[0] if len(sessions) == 1 else None


def run_auto_action(name: str, last_kicad: dict, kind_out: Path, disp: str) -> None:
    """The watcher's half of an auto-attach gate, run BEFORE the script's shot is released (the script waits on `<name>.done`).

    `auto-act-type-<tag>`: real X keys (XTest) typing the marker as a person does (`mid` also records KiCad's hub state when the typing
    began and ended); `auto-act-stale-future|old`: an `editor.kicad.attach` with a generation that does not exist yet (running+7) or the
    previous KiCad's, with the complete session state captured right before and right after it and a control with the running
    generation; `auto-act-close`: KiCad's own Close on its socket (the person's Quit as the hub sees it); `auto-act-kill`: a crash
    (SIGKILL of the KiCad pid the hub names), which is not the person's close. Every outcome is a record in `auto_actions.jsonl`; a
    failure is recorded, never raised.
    """
    repo_root = Path(__file__).resolve().parents[2]
    xinput = repo_root / "test" / "cef" / "xinput.py"
    ipc = repo_root / "test" / "kicad_embed" / "lib" / "gui_ipc_probe.py"
    hub_rpc = repo_root / "test" / "kicad_embed" / "lib" / "hub_rpc.mjs"
    port, token, bun, pro = (os.environ.get(k, "") for k in ("RAIL_HUB_PORT", "RAIL_HUB_TOKEN_FILE", "RAIL_BUN_BIN", "RAIL_PRO"))
    rec: dict = {"shot": name, "ok": False, "error": None}
    try:
        if name.startswith("auto-act-type-"):
            tag = name[len("auto-act-type-"):]
            text = AUTO.MARKERS[tag]
            rec.update({"action": f"type-{tag}", "text": text})
            if tag == "mid":
                sess = _single_session(port, token, bun)
                rec["kicad_state_before"] = (sess or {}).get("state")
            # a person typing in the composer has the pointer over Backplane's own window (the window manager follows the pointer for
            # focus): leave it there, not over the embedded KiCad where an earlier click left it (that would hand KiCad the keys)
            subprocess.run([sys.executable, str(xinput), "move", str(COMPOSER_POINTER[0]), str(COMPOSER_POINTER[1])], env=dict(os.environ, DISPLAY=disp), check=True, timeout=10)
            time.sleep(0.2)
            subprocess.run([sys.executable, str(xinput), "type", text], env=dict(os.environ, DISPLAY=disp), check=True, timeout=30)
            time.sleep(0.4)
            if tag == "mid":
                sess = _single_session(port, token, bun)
                rec["kicad_state_after"] = (sess or {}).get("state")
            rec["ok"] = True
        elif name in ("auto-act-stale-future", "auto-act-stale-old"):
            label = name[len("auto-act-stale-"):]
            rec.update({"action": f"stale-{label}"})
            if not pro or not hub_rpc.exists():
                raise RuntimeError("no project path / hub helper")
            sess = _single_session(port, token, bun)
            before = AUTO.state_of(sess)
            running = ((sess or {}).get("runtime") or {}).get("generation")
            if not isinstance(running, int):
                raise RuntimeError("no single session with a runtime generation")
            if label == "future":
                _FIRST_GENERATION[:] = [running]
                sent = running + 7
            else:
                if not _FIRST_GENERATION or _FIRST_GENERATION[0] >= running:
                    raise RuntimeError(f"no previous generation exists (first {list(_FIRST_GENERATION)}, running {running})")
                sent = _FIRST_GENERATION[0]
            rec.update({"generation": running, "sent_generation": sent})
            env = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY")}

            def rpc(method: str, params: dict) -> dict | None:
                out = subprocess.check_output([bun, str(hub_rpc), port, token, json.dumps([{"m": method, "p": params}])], env=env, text=True, timeout=20)
                for line in out.splitlines():
                    if line.startswith("{"):
                        try:
                            r = json.loads(line)
                        except Exception:
                            continue
                        if r.get("m") == method:
                            return r
                return None

            def stale_flag(r: dict | None) -> bool | None:
                return None if r is None else bool((r.get("result") or {}).get("stale") is True or (r.get("reply") or {}).get("stale") is True)

            old = rpc("editor.kicad.attach", {"project": pro, "frame": "PCB", "wait": False, "auto": True, "generation": sent})
            after = AUTO.state_of(_single_session(port, token, bun))  # immediately after the stale attach, before the control
            cur = rpc("editor.kicad.show", {"project": pro, "frame": "PCB", "generation": running})
            rec.update({"before": before, "after": after, "stale": stale_flag(old), "control_stale": stale_flag(cur),
                        "control_ok": bool(cur is not None and cur.get("ok") is True and not cur.get("error")), "control_error": None if cur is None else (cur.get("error") or None)})
            rec["ok"] = old is not None
        elif name == "auto-act-close":
            rec.update({"action": "close"})
            sock = last_kicad.get("sock")
            if not sock:
                raise RuntimeError("the last pane report carries no KiCad socket")
            r = subprocess.run([sys.executable, str(ipc), "close", str(sock), "discard"], capture_output=True, text=True, timeout=30)
            rec["output"] = (r.stdout or r.stderr or "").strip()[-600:]
            rec["ok"] = r.returncode == 0 and '"ok": true' in (r.stdout or "").replace('"ok":true', '"ok": true')
        elif name == "auto-act-kill":
            rec.update({"action": "kill"})
            sess = _single_session(port, token, bun)
            pid = (sess or {}).get("pid")
            if not isinstance(pid, int) or pid <= 1:
                raise RuntimeError("the hub names no single KiCad pid to kill")
            cmdline = Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode(errors="replace")
            if "kicad" not in cmdline:
                raise RuntimeError(f"pid {pid} is not a KiCad process")
            os.kill(pid, 9)
            for _ in range(50):  # the process is gone before the stale files are cleared
                if not Path(f"/proc/{pid}").exists():
                    break
                time.sleep(0.1)
            # A real crash leaves KiCad's lock and autosave files behind, and the next start then asks about them in a modal dialog that
            # (rightly) takes the keyboard. That is the crash-recovery path, not the Automatic attach under test: clear them from the
            # run's own throwaway project so the restart is a plain cold start.
            cleared = []
            if pro:
                for pat in ("~*.lck", "_autosave-*", "*-backups"):
                    for f in Path(pro).parent.glob(pat):
                        if f.is_file():
                            f.unlink()
                            cleared.append(f.name)
            rec.update({"pid": pid, "ok": True, "stale_files_cleared": cleared})
        else:
            return
    except Exception as e:
        rec["error"] = str(e)[:300]
    with (kind_out / "auto_actions.jsonl").open("a", encoding="utf-8") as af:
        af.write(json.dumps(rec, separators=(",", ":")) + "\n")


def watch_and_capture(disp: str, client_log: Path, shot_dir: Path, kind_out: Path, stop_flag: Path) -> int:
    """Tail client_log for shot-ready markers, capture with `import -window root`, touch <name>.done."""
    captures_path = kind_out / "captures.jsonl"
    seen: set[str] = set()
    seen_marks: set[str] = set()
    last_kicad: dict = {}
    probes = {}
    pos = 0
    buf = ""
    env = dict(os.environ, DISPLAY=disp)

    while True:
        if client_log.exists():
            with client_log.open("r", encoding="utf-8", errors="replace") as f:
                f.seek(pos)
                chunk = f.read()
                pos = f.tell()
            if chunk:
                buf += chunk
                while "\n" in buf:
                    line, buf = buf.split("\n", 1)
                    if "GPUI_METRIC " not in line:
                        continue
                    try:
                        ev = json.loads(line.split("GPUI_METRIC ", 1)[1])
                    except Exception:
                        continue
                    etype = ev.get("event")
                    if etype == "probes":
                        rows = ev.get("probes") or ev.get("rects") or []
                        probes = rows if isinstance(rows, dict) else {r["id"]: r for r in rows}
                    if etype == "kicad":
                        last_kicad = ev
                        continue
                    if etype == "mark" and ev.get("name") == "embed:sim-run" and "embed:sim-run" not in seen_marks:
                        seen_marks.add("embed:sim-run")
                        trigger_sim_run(disp, last_kicad, kind_out)
                        continue
                    if etype != "shot-ready":
                        continue
                    name = ev.get("name") or ""
                    if not SHOT_NAME_RE.match(name) or name in seen:
                        continue
                    seen.add(name)
                    if AUTO.is_action_only_shot(name):
                        # an action-only shot saves NO screenshot (nothing to review, nothing to leak): the watcher acts, then releases the script
                        run_auto_action(name, last_kicad, kind_out, disp)
                        done_tmp = shot_dir / f".{name}.done.tmp"
                        done_tmp.write_text("ok\n", encoding="utf-8")
                        done_tmp.replace(shot_dir / f"{name}.done")
                        continue
                    out_png = kind_out / f"{name}.png"
                    ok = False
                    err = None
                    try:
                        subprocess.run(["import", "-window", "root", str(out_png)], env=env, check=True, timeout=15)
                        ok = out_png.exists() and out_png.stat().st_size > 0
                        if not ok:
                            err = "empty png"
                    except Exception as e:
                        err = str(e)
                    obstructions, inspect_ok, inspect_error = detect_obstructions(disp)
                    hub_sessions = None
                    if not name.endswith("-300ms"):
                        hub_sessions = query_hub_kicad_sessions(
                            os.environ.get("RAIL_HUB_PORT", ""),
                            os.environ.get("RAIL_HUB_TOKEN_FILE", ""),
                            os.environ.get("RAIL_BUN_BIN", ""),
                        )
                    rec = {
                        "name": name,
                        "path": str(out_png),
                        "ok": ok,
                        "bytes": out_png.stat().st_size if out_png.exists() else 0,
                        "error": err,
                        "inspect_ok": inspect_ok,
                        "inspect_error": inspect_error,
                        "obstructed": len(obstructions) > 0,
                        "obstructions": obstructions,
                        "hub_kicad_sessions": hub_sessions,
                        "state": ev.get("state", ""),
                        "settled": ev.get("settled", False),
                    }
                    if os.environ.get("RAIL_B2_SIZE") == "minimum" and ok and inspect_ok and not obstructions:
                        page = name.split("-menu")[0]
                        target = f"pane-more:{page}"
                        if name == "gerbers-menu":
                            target = "pane-menu-row:gerbers:gerbers-vs-board"
                            if target not in probes:
                                target = "pane-tool:gerbers:gerbers-vs-board"
                        if name == page or name == "gerbers-menu":
                            rect = probes.get(target)
                            if rect:
                                x, y = (round(rect[k] + rect[d]/2) for k, d in (("x", "w"), ("y", "h")))
                                helper = Path(__file__).resolve().parents[1] / "cef" / "xinput.py"
                                click = subprocess.run([sys.executable, str(helper), "click", str(x), str(y)],
                                                       env=env, timeout=10)
                                rec["click"] = {"target": target, "point": [x, y], "ok": click.returncode == 0}
                    with captures_path.open("a", encoding="utf-8") as cf:
                        cf.write(json.dumps(rec, separators=(",", ":")) + "\n")
                    done_tmp = shot_dir / f".{name}.done.tmp"
                    done_tmp.write_text("ok\n", encoding="utf-8")
                    done_tmp.replace(shot_dir / f"{name}.done")
        if stop_flag.exists():
            # Drain any final lines once more then exit
            if client_log.exists() and client_log.stat().st_size > pos:
                continue
            break
        time.sleep(0.03)
    return 0


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: rail_shots_driver.py (gen-script KIND OUT_JSON [EARLY_MS] [TIMEOUT_MS] [--embed] | watch DISP CLIENT_LOG SHOT_DIR KIND_OUT STOP_FLAG)", file=sys.stderr)
        return 2
    cmd = sys.argv[1]
    if cmd == "gen-script":
        args = [a for a in sys.argv[2:] if a != "--embed"]
        embed = "--embed" in sys.argv[2:]
        kind = args[0]
        out_json = Path(args[1])
        early_ms = int(args[2]) if len(args) > 2 else 300
        timeout_ms = int(args[3]) if len(args) > 3 else 15000
        steps = build_script(kind, early_ms=early_ms, settle_timeout_ms=timeout_ms, embed=embed)
        out_json.write_text(json.dumps(steps, indent=2) + "\n", encoding="utf-8")
        return 0
    if cmd == "watch":
        return watch_and_capture(sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]), Path(sys.argv[6]))
    print(f"unknown subcommand: {cmd}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
