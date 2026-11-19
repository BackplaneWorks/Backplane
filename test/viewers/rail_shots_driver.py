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
import rail_shots_manifest as M  # noqa: E402

SHOT_NAME_RE = re.compile(r"^[a-z0-9-]+$")


def build_script(kind: str, early_ms: int = 300, settle_timeout_ms: int = 15000) -> list[dict]:
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
                # Task 5: tap a part on the board pane (slot 2) in Split and check cross-probe
                steps.extend([
                    {"mark": "press:task5-split-probe"},
                    {"tap": [0.352, 0.857, 2]},
                    {"wait": 600},
                    {"mark": "settled:task5-split-probe"},
                    {"shot": "task5-split-probe", "settle": "split", "timeout_ms": settle_timeout_ms},
                ])
            elif slug == "board":
                # Task 9: press Follow agent on Board and verify viewer.follow.on + visible state
                steps.extend([
                    {"mark": "press:task9-follow-on"},
                    {"press": "follow-agent"},
                    {"wait": 600},
                    {"mark": "settled:task9-follow-on"},
                    {"shot": "task9-follow-on", "settle": "board", "timeout_ms": settle_timeout_ms},
                ])

    if kind == "full":
        # Task 3: verify 1-press return to Schematic from every other page
        for src_slug, src_rail, src_settle in M.TASK3_SOURCES:
            steps.extend([
                {"mark": f"task3-setup:{src_slug}"},
                {"press": src_rail},
                {"wait": 400},
                {"mark": f"press:task3-from-{src_slug}"},
                {"press": "rail:Schematic"},
                {"mark": f"settled:task3-from-{src_slug}"},
                {"shot": f"task3-from-{src_slug}", "settle": "schematic", "timeout_ms": settle_timeout_ms},
            ])
        # Task 10: close the viewer from an open page (Schematic -> press rail:Schematic) and verify idle width
        steps.extend([
            {"mark": "press:task10-closed"},
            {"press": "rail:Schematic"},
            {"mark": "settled:task10-closed"},
            {"shot": "task10-closed", "settle": "", "timeout_ms": settle_timeout_ms},
        ])

    return steps


def detect_obstructions(disp: str) -> list[dict]:
    """Inspect X11 top-level windows on `disp` for native dialogs/windows obstructing Backplane."""
    env = dict(os.environ, DISPLAY=disp)
    try:
        out = subprocess.check_output(["xwininfo", "-root", "-tree"], env=env, text=True, stderr=subprocess.DEVNULL, timeout=5)
    except Exception:
        return []
    obstructions: list[dict] = []
    # Match lines like:
    #   0x600003 "Warning": ("kicad" "Kicad")  610x120+415+390  +415+390
    #   0x600003 (has no name): ("kicad" "Kicad")  610x120+415+390  +415+390
    win_re = re.compile(r'^\s*(0x[0-9a-f]+)\s+(?:"([^"]*)"|\(has no name\)):\s+\("([^"]*)"\s+"([^"]*)"\)\s+(\d+)x(\d+)\+(-?\d+)\+(-?\d+)')
    for line in out.splitlines():
        m = win_re.match(line)
        if not m:
            continue
        xid, title, cls1, cls2, w_s, h_s, x_s, y_s = m.groups()
        title = title or ""
        w, h = int(w_s), int(h_s)
        if w < 60 or h < 40:
            continue
        low_cls = f"{cls1} {cls2}".lower()
        low_title = title.lower()
        # Ignore i3 WM frames and Backplane's own GPUI window
        if "i3" in low_cls or "backplane" in low_cls or "backplane" in low_title:
            continue
        # Confirm it is actually mapped (IsViewable)
        try:
            info = subprocess.check_output(["xwininfo", "-id", xid], env=env, text=True, stderr=subprocess.DEVNULL, timeout=2)
            if "Map State: IsViewable" not in info:
                continue
        except Exception:
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
    return obstructions


def watch_and_capture(disp: str, client_log: Path, shot_dir: Path, kind_out: Path, stop_flag: Path) -> int:
    """Tail client_log for shot-ready markers, capture with `import -window root`, touch <name>.done."""
    captures_path = kind_out / "captures.jsonl"
    seen: set[str] = set()
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
                    if "GPUI_METRIC " not in line or '"shot-ready"' not in line:
                        continue
                    try:
                        ev = json.loads(line.split("GPUI_METRIC ", 1)[1])
                    except Exception:
                        continue
                    if ev.get("event") != "shot-ready":
                        continue
                    name = ev.get("name") or ""
                    if not SHOT_NAME_RE.match(name) or name in seen:
                        continue
                    seen.add(name)
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
                    obstructions = detect_obstructions(disp)
                    rec = {
                        "name": name,
                        "path": str(out_png),
                        "ok": ok,
                        "bytes": out_png.stat().st_size if out_png.exists() else 0,
                        "error": err,
                        "obstructed": len(obstructions) > 0,
                        "obstructions": obstructions,
                        "state": ev.get("state", ""),
                        "settled": ev.get("settled", False),
                    }
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
        print("usage: rail_shots_driver.py (gen-script KIND OUT_JSON [EARLY_MS] [TIMEOUT_MS] | watch DISP CLIENT_LOG SHOT_DIR KIND_OUT STOP_FLAG)", file=sys.stderr)
        return 2
    cmd = sys.argv[1]
    if cmd == "gen-script":
        kind = sys.argv[2]
        out_json = Path(sys.argv[3])
        early_ms = int(sys.argv[4]) if len(sys.argv) > 4 else 300
        timeout_ms = int(sys.argv[5]) if len(sys.argv) > 5 else 15000
        steps = build_script(kind, early_ms=early_ms, settle_timeout_ms=timeout_ms)
        out_json.write_text(json.dumps(steps, indent=2) + "\n", encoding="utf-8")
        return 0
    if cmd == "watch":
        return watch_and_capture(sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]), Path(sys.argv[6]))
    print(f"unknown subcommand: {cmd}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
