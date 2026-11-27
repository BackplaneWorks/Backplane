#!/usr/bin/env python3
"""Isolated video-verified E2E + anti-flicker suite for the embedded SolidCraft CAD pane.

Spins up an isolated Backplane hub (--home in $BP_TMP, dedicated --port, --no-tailscale),
stand-in agents (BACKPLANE_AGENTS), and a separate GPUI desktop client inside a private Xvfb + i3
window manager. Never touches ~/.backplane-gpui-fresh or live user data.

Usage:
  solidcraft-pane-e2e.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [--step PATH] [--port PORT] [--display N]

Outputs:
  OUT/video.mp4        30 fps x11grab capture of the isolated Xvfb display
  OUT/frames/*.png     extracted video frames
  OUT/keyframes/*.png  labeled keyframes for visual inspection
  OUT/report.json      structured check results + frame-diff / anti-flicker metrics
"""

import argparse
import glob
import http.server
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

from PIL import Image, ImageChops, ImageStat

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
XIN = os.path.join(REPO, "test", "cef", "xinput.py")

W, H, FPS = 1440, 900, 30


def find_xvfb():
    for cand in [
        os.environ.get("XVFB", ""),
        os.path.expanduser("~/.local/xvfb-root/usr/bin/Xvfb"),
        os.path.expanduser("~/.cache/xvfb-root/usr/bin/Xvfb"),
        shutil.which("Xvfb") or "",
    ]:
        if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
            root = os.path.dirname(os.path.dirname(os.path.dirname(cand)))
            lib = os.environ.get("XVFB_LIB") or os.path.join(root, "usr", "lib", "x86_64-linux-gnu")
            return cand, lib if os.path.isdir(lib) else ""
    sys.exit("Xvfb not found (checked ~/.local/xvfb-root, ~/.cache/xvfb-root, PATH)")


def find_solidcraft_bin_dir():
    for cand in [
        os.environ.get("SOLIDCRAFT_BIN_DIR", ""),
        os.environ.get("BACKPLANE_SOLIDCRAFT", ""),
        os.path.dirname(shutil.which("solidcraft") or ""),
    ]:
        if cand and os.path.isfile(os.path.join(cand, "solidcraft")) and os.path.isfile(os.path.join(cand, "solidcraft-cli")):
            return cand
        if cand and os.path.isfile(os.path.join(cand, "bin", "solidcraft")) and os.path.isfile(os.path.join(cand, "bin", "solidcraft-cli")):
            return os.path.join(cand, "bin")
    return ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("client_bin")
    ap.add_argument("hub_bin")
    ap.add_argument("assets_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--step", default="")
    ap.add_argument("--port", type=int, default=3896)
    ap.add_argument("--display", type=int, default=89)
    args = ap.parse_args()

    CLIENT = os.path.abspath(args.client_bin)
    HUB = os.path.abspath(args.hub_bin)
    ASSETS = os.path.abspath(args.assets_dir)
    OUT = os.path.abspath(args.out_dir)
    PORT = args.port
    DISP = f":{args.display}"

    if os.path.exists(f"/tmp/.X11-unix/X{args.display}"):
        sys.exit(f"display {DISP} is already in use")

    XV, XLIB = find_xvfb()
    SC_DIR = find_solidcraft_bin_dir()

    shutil.rmtree(OUT, ignore_errors=True)
    P = lambda *a: os.path.join(OUT, *a)
    for d in ("home", "proj/mech", "proj/docs", "bin", "fakehome/.cache", "fakehome/.local/share", "frames", "keyframes"):
        os.makedirs(P(d), exist_ok=True)

    RUN = tempfile.mkdtemp(prefix="sc.")
    os.chmod(RUN, 0o700)

    # Stand-in agents only
    shutil.copy(os.path.join(HERE, "lib", "standin_rows.py"), P("bin", "claude"))
    os.chmod(P("bin", "claude"), 0o755)
    for n in ("codex", "grok", "muse", "opencode", "agy", "pi", "omp"):
        with open(P("bin", n), "w") as fh:
            fh.write("#!/bin/sh\nexit 1\n")
        os.chmod(P("bin", n), 0o755)

    # Project fixtures: KiCad board/sch + real STEP + generated .sc
    step_src = args.step if os.path.isfile(args.step) else os.path.join(REPO, "test/files/fixtures/mesh/bracket.step")
    shutil.copy(step_src, P("proj/mech/case.step"))
    for ext in ("kicad_pcb", "kicad_sch"):
        shutil.copy(os.path.join(REPO, f"test/files/fixtures/plot/ecc83-pp.{ext}"), P(f"proj/ecc83.{ext}"))
    with open(P("proj/ecc83.kicad_pro"), "w") as fh:
        fh.write("{}\n")
    with open(P("proj/docs/notes.txt"), "w") as fh:
        fh.write("SolidCraft E2E test notes\n")

    if SC_DIR:
        cli = os.path.join(SC_DIR, "solidcraft-cli")
        script_path = P("make_box.json")
        with open(script_path, "w") as fh:
            json.dump(
                {
                    "commands": [
                        {"command": "parameters.add", "params": {"name": "w", "expression": "40 mm"}},
                        {"command": "solid.box", "params": {"length": "w", "width": 25, "height": 12, "name": "Base"}},
                        {"command": "solid.cylinder", "params": {"radius": 3, "height": 12, "operation": "cut", "name": "Hole"}},
                    ]
                },
                fh,
            )
        box_env = dict(os.environ, SOLIDCRAFT_NO_TELEMETRY="1", SOLIDCRAFT_UPDATE_CHECK="0")
        box_res = subprocess.run([cli, "run", script_path, "--save", P("proj/mech/box.sc"), "--out", P("proj/mech/box.stl")], env=box_env, check=False, capture_output=True, text=True)
        if box_res.returncode != 0 or not os.path.isfile(P("proj/mech/box.sc")):
            sys.exit(f"failed to build box.sc fixture (rc={box_res.returncode}): {box_res.stderr or box_res.stdout}")

    now = int(time.time())
    seed = [
        {"$": "ProjectCreated", "id": "p1", "title": "solidcraft-e2e", "root": P("proj"), "at": now},
        {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "CAD Work", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now + 1},
        {"$": "ThreadPinned", "id": "t1", "pinned": True, "at": now + 2},
        {"$": "SettingSet", "key": "onboard.done", "value": "1", "at": now + 3},
        {"$": "SettingSet", "key": "restart.continue", "value": "off", "at": now + 4},
    ]
    with open(P("home/events.jsonl"), "w") as fh:
        for e in seed:
            fh.write(json.dumps(e) + "\n")

    # MCP call batches: batch 1 edits box.sc live in the bridged pane; batch 2 tests file tools + viewer_render + viewer_show.
    mcp_batch_1 = [
        {"name": "solidcraft_status", "arguments": {}},
        {"name": "solidcraft_call", "arguments": {"tool": "open", "arguments": {"path": "mech/box.sc"}}},
        {"name": "solidcraft_summary", "arguments": {}},
        {"name": "solidcraft_call", "arguments": {"tool": "set_parameter", "arguments": {"name": "w", "expression": "90 mm"}}},
        {"name": "solidcraft_look", "arguments": {"views": ["iso"]}},
    ]
    mcp_batch_2 = [
        {"name": "files_list", "arguments": {"path": ""}},
        {"name": "files_write", "arguments": {"path": "mech/spec.txt", "text": "housing v1\n"}},
        {"name": "files_read", "arguments": {"path": "mech/spec.txt"}},
        {"name": "files_read", "arguments": {"path": "mech/box.sc"}},
        {"name": "files_show", "arguments": {"path": "mech"}},
        {"name": "viewer_render", "arguments": {"path": "mech/case.step", "view": "iso"}},
        {"name": "viewer_show", "arguments": {"kind": "mech", "path": "mech/case.step"}},
    ]
    with open(P("mcp_calls.json"), "w") as fh:
        json.dump(mcp_batch_1 if SC_DIR else mcp_batch_2, fh)

    procs = []
    checks = []
    pane_r = {}

    def log(cid, ok, ev):
        checks.append({"id": cid, "ok": bool(ok), "ev": ev})
        print(("ok   " if ok else "FAIL ") + cid + " " + ev, flush=True)

    env = dict(os.environ)
    for k in ("I3SOCK", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY"):
        env.pop(k, None)
    path_parts = [P("bin")]
    if SC_DIR:
        path_parts.append(SC_DIR)
        env["BACKPLANE_SOLIDCRAFT"] = SC_DIR
    path_parts.append(env.get("PATH", ""))
    env.update(
        XDG_RUNTIME_DIR=RUN,
        LC_ALL="C",
        BACKPLANE_AGENTS=P("bin"),
        BACKPLANE_NO_UPDATE="1",
        SHELL="/bin/sh",
        FAKE_LOG=P("fake.log"),
        FAKE_SECS="10",
        BP_MCP_CALLS_FILE=P("mcp_calls.json"),
        BP_STANDIN_IMAGES=P("keyframes"),
        HOME=P("fakehome"),
        XDG_CACHE_HOME=P("fakehome/.cache"),
        XDG_CONFIG_HOME=P("fakehome/.config"),
        XDG_DATA_HOME=P("fakehome/.local/share"),
        SOLIDCRAFT_SETTINGS_DIR=P("fakehome/.config/solidcraft"),
        PATH=":".join(path_parts),
    )

    def spawn(cmd, log_name, e=None, cwd=None):
        p = subprocess.Popen(cmd, env=e or env, cwd=cwd, stdout=open(P(log_name), "ab"), stderr=subprocess.STDOUT, start_new_session=True)
        procs.append(p)
        return p

    def cleanup(*_):
        for p in reversed(procs):
            try:
                os.killpg(p.pid, signal.SIGTERM)
            except Exception:
                pass
        shutil.rmtree(RUN, ignore_errors=True)

    signal.signal(signal.SIGTERM, lambda *_: (cleanup(), sys.exit(1)))

    try:
        hub = spawn([HUB, "--home", P("home"), "--port", str(PORT), "--host", "127.0.0.1", "--no-tailscale", "--foreground"], "hub.log", cwd=P("proj"))
        for _ in range(120):
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{PORT}/hello", timeout=1)
                break
            except Exception:
                time.sleep(0.25)

        xenv = dict(env, LD_LIBRARY_PATH=XLIB) if XLIB else dict(env)
        spawn([XV, DISP, "-screen", "0", f"{W}x{H}x24", "-nolisten", "tcp", "-xkbdir", "/usr/share/X11/xkb"], "xvfb.log", xenv)
        time.sleep(1.5)

        with open(P("i3.conf"), "w") as fh:
            fh.write(f"font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\nfor_window [title=\"^SolidCraft\"] floating enable\ndefault_border none\nhide_edge_borders both\nipc-socket {RUN}/i3.sock\n")
        denv = dict(env, DISPLAY=DISP)
        spawn(["i3", "-c", P("i3.conf")], "i3.log", denv)
        time.sleep(1.0)

        vk = (sorted(glob.glob("/usr/share/vulkan/icd.d/lvp_icd*.json")) or ["/usr/share/vulkan/icd.d/lvp_icd.json"])[0]
        cenv = dict(denv, VK_ICD_FILENAMES=vk, LP_NUM_THREADS="2", BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS="1")

        # Keep script open and drive real interactions + metrics + SolidCraft script hooks
        race_steps = [
            {"select": 0},
            {"wait": 3000},
            # Attach case.step (enters starting), immediately switch to box.sc before ready -> must not respawn (same pid/gen)
            {"viewshow": ["mech", P("proj/mech/case.step")]},
            {"wait": 600},
            {"solidcraft": {"report": True, "tag": "while_starting"}},
            {"solidcraft": {"open": P("proj/mech/box.sc"), "tag": "open_while_starting"}},
            {"solidcraft": {"wait_ready": 45000, "min_painted": 5, "tag": "ready_after_race"}},
            {"wait": 1500},
            {"solidcraft": {"report": True, "tag": "post_race_file"}},
            {"wait": 300000},
        ]
        with open(P("script.json"), "w") as fh:
            json.dump(race_steps, fh)

        t_rec = time.time()
        rec = spawn(
            [
                "ffmpeg", "-v", "error", "-y",
                "-f", "x11grab", "-framerate", str(FPS), "-video_size", f"{W}x{H}", "-i", DISP,
                "-c:v", "libx264", "-preset", "ultrafast", "-crf", "10",
                P("video.mp4"),
            ],
            "ffmpeg.log",
            denv,
        )
        client = spawn([CLIENT, "--url", f"http://127.0.0.1:{PORT}", "--token-file", P("home/token"), "--script", P("script.json"), "--exit-after", "300"], "client.log", cenv)
        for _ in range(120):
            ms = [json.loads(l.split("GPUI_METRIC ", 1)[1]) for l in open(P("client.log"), errors="replace") if "GPUI_METRIC " in l] if os.path.isfile(P("client.log")) else []
            if any(m.get("event") == "solidcraft" and m.get("tag") == "post_race_file" for m in ms):
                break
            time.sleep(0.5)

        sc_events = [json.loads(l.split("GPUI_METRIC ", 1)[1]) for l in open(P("client.log"), errors="replace") if "GPUI_METRIC " in l]
        ev_start = next((m for m in sc_events if m.get("event") == "solidcraft" and m.get("tag") == "while_starting"), {})
        ev_ready = next((m for m in sc_events if m.get("event") == "solidcraft" and m.get("tag") == "ready_after_race"), {})
        ev_post = next((m for m in sc_events if m.get("event") == "solidcraft" and m.get("tag") == "post_race_file"), {})
        final_file = ev_post.get("hub_file") or ev_post.get("file") or ""
        same_proc = (
            ev_post.get("state") == "ready"
            and ev_start.get("pid", 0) > 0
            and ev_post.get("gen", 0) > 0
            and ev_start.get("pid") == ev_post.get("pid")
            and ev_start.get("gen") == ev_post.get("gen")
            and final_file.endswith("box.sc")
        )
        log("open_while_starting_no_respawn", same_proc, f"start={ev_start.get('pid')}/{ev_start.get('gen')} ready={ev_ready.get('state')} post={ev_post.get('pid')}/{ev_post.get('gen')} file={final_file}")

        def metrics():
            if not os.path.isfile(P("client.log")):
                return []
            return [json.loads(l.split("GPUI_METRIC ", 1)[1]) for l in open(P("client.log"), errors="replace") if "GPUI_METRIC " in l]

        def rects():
            r = {}
            for m in metrics():
                ev = m.get("event")
                if ev == "layout" and isinstance(m.get("rects"), dict):
                    r.update(m["rects"])
                elif ev == "probes" and isinstance(m.get("rects"), list):
                    for item in m["rects"]:
                        if isinstance(item, dict) and "id" in item:
                            r[item["id"]] = item
            return r

        def latest_probes():
            r = {}
            for m in metrics():
                if m.get("event") == "probes" and isinstance(m.get("rects"), list):
                    r = {item["id"]: item for item in m["rects"] if isinstance(item, dict) and "id" in item}
            return r

        def click_rect(key):
            r = rects().get(key)
            if not r:
                return False
            cx = r["x"] + r["w"] / 2.0
            cy = r["y"] + r["h"] / 2.0
            subprocess.run(["python3", XIN, "click", str(int(cx)), str(int(cy))], env=denv, check=False, timeout=10)
            return True

        def key_shot(name):
            out = P("keyframes", f"{name}.png")
            subprocess.run(["import", "-window", "root", out], env=denv, check=False, timeout=10)
            return out

        def x11_solidcraft_windows():
            res = subprocess.run(["xwininfo", "-root", "-tree"], env=denv, capture_output=True, text=True, timeout=5)
            hits = []
            for line in res.stdout.splitlines():
                if "io.tangencylabs.solidcraft" in line or "SolidCraft" in line:
                    wid = line.strip().split()[0]
                    info = subprocess.run(["xwininfo", "-id", wid], env=denv, capture_output=True, text=True, timeout=5).stdout
                    geo = {}
                    for l2 in info.splitlines():
                        l2 = l2.strip()
                        if l2.startswith("Absolute upper-left X:"):
                            geo["x"] = int(l2.split(":", 1)[1].strip())
                        elif l2.startswith("Absolute upper-left Y:"):
                            geo["y"] = int(l2.split(":", 1)[1].strip())
                        elif l2.startswith("Width:"):
                            geo["w"] = int(l2.split(":", 1)[1].strip())
                        elif l2.startswith("Height:"):
                            geo["h"] = int(l2.split(":", 1)[1].strip())
                    hits.append({"wid": wid, "line": line.strip(), "viewable": "IsViewable" in info, "info": info, "geo": geo})
            return hits

        phase_marks = {}
        def mark_phase(name):
            phase_marks[name] = time.time() - t_rec

        # 1. With box.sc ready in the embedded pane, run Batch 1 (bridged MCP live-edit: solidcraft_status/summary/set_parameter/look)
        viewer_box = (int(W * 0.48), int(H * 0.12), int(W * 0.96), int(H * 0.92))
        def run_zqcalls(wait_marker, timeout_s=45):
            subprocess.run(["python3", XIN, "focus", "Backplane"], env=denv, check=False, timeout=10)
            if click_rect("composer"):
                subprocess.run(["python3", XIN, "type", "zqcalls"], env=denv, check=False, timeout=15)
                subprocess.run(["python3", XIN, "key", "Return"], env=denv, check=False, timeout=10)
            deadline = time.time() + timeout_s
            while time.time() < deadline:
                txt = open(P("fake.log"), errors="replace").read() if os.path.isfile(P("fake.log")) else ""
                if wait_marker in txt:
                    time.sleep(1.5)
                    return txt
                time.sleep(0.5)
            return open(P("fake.log"), errors="replace").read() if os.path.isfile(P("fake.log")) else ""

        if SC_DIR:
            time.sleep(2.0)
            shot_before = key_shot("key_mcp_before")
            wins_before = [w["wid"] for w in x11_solidcraft_windows() if w["viewable"]]
            fake_log = run_zqcalls("CALL solidcraft_look ")
            shot_after = key_shot("key_mcp_after")
            wins_after = [w["wid"] for w in x11_solidcraft_windows() if w["viewable"]]
            img_b = Image.open(shot_before).convert("L").crop(viewer_box)
            img_a = Image.open(shot_after).convert("L").crop(viewer_box)
            mcp_diff = ImageStat.Stat(ImageChops.difference(img_b, img_a)).mean[0]
            sc_lines = [l for l in fake_log.splitlines() if "CALL solidcraft_" in l]
            status_line = next((l for l in sc_lines if "CALL solidcraft_status ok" in l), "")
            summary_line = next((l for l in sc_lines if "CALL solidcraft_summary ok" in l), "")
            mcp_live_ok = (
                "pane" in status_line
                and '"name":"w"' in summary_line
                and "CALL solidcraft_call ok" in fake_log
                and "CALL solidcraft_look ok" in fake_log
                and len(wins_before) == 1
                and wins_before == wins_after
                and mcp_diff > 0.05
            )
            log("mcp_solidcraft_live_edit", mcp_live_ok, f"wins={wins_before}->{wins_after} diff={mcp_diff:.3f} calls={'; '.join(sc_lines)[:260]}")
            with open(P("mcp_calls.json"), "w") as fh:
                json.dump(mcp_batch_2, fh)

        # 2. Run Batch 2 (`zqcalls`) to exercise project file tools + viewer_render + viewer_show (mech/case.step)
        fake_log = run_zqcalls("CALL viewer_show ", timeout_s=45)
        log("file_tools_mcp", "CALL files_list ok" in fake_log and "CALL files_write ok" in fake_log and "CALL files_read ok" in fake_log and "CALL files_read error" in fake_log,
            "stand-in MCP file tools logged: " + "; ".join([l for l in fake_log.splitlines() if "CALL " in l])[:300])
        log("viewer_render_cli", "CALL viewer_render ok" in fake_log, "viewer_render result in fake.log")

        # 3. Switch to Files tab and back to SolidCraft Mech (case.step), verify embedded SolidCraft window
        mark_phase("files_tab")
        files_clicked = click_rect("rail:Files")
        time.sleep(1.5)
        key_shot("key_files")

        mark_phase("solidcraft_open")
        mech_clicked = click_rect("rail:Mech")
        time.sleep(3.5)
        key_shot("key_step_open")
        sc_wins = x11_solidcraft_windows()
        log("solidcraft_embedded", files_clicked and mech_clicked and any(w["viewable"] for w in sc_wins), f"clicked=({files_clicked},{mech_clicked}) SolidCraft X11 windows: {[w['line'] for w in sc_wins]}")

        # 4. Switch across KiCad Board -> SolidCraft Mech and verify zero Z-order bleed
        mark_phase("kicad_tab")
        board_clicked = click_rect("rail:Board")
        time.sleep(2.0)
        key_shot("key_kicad_tab")
        sc_hidden_on_board = not any(w["viewable"] for w in x11_solidcraft_windows())
        log("no_zorder_bleed_board", board_clicked and sc_hidden_on_board, f"board_clicked={board_clicked} SolidCraft hidden on Board tab: {sc_hidden_on_board}")

        mark_phase("solidcraft_return")
        click_rect("rail:Mech")
        time.sleep(2.5)
        key_shot("key_solidcraft_return")

        # 5. Stepped resize (15 shrink steps + 15 grow steps via i3-msg) + split-pane drag + settle + rect alignment
        mark_phase("resize_start")
        i3sock = f"{RUN}/i3.sock"
        subprocess.run(["i3-msg", "-s", i3sock, "floating", "enable"], env=denv, check=False, timeout=5)
        for step in range(1, 16):
            rw = W - step * 20
            rh = H - step * 10
            subprocess.run(["i3-msg", "-s", i3sock, "resize", "set", str(rw), str(rh)], env=denv, check=False, timeout=5)
            time.sleep(0.06)
        time.sleep(0.5)
        key_shot("key_resized")
        for step in range(14, -1, -1):
            rw = W - step * 20
            rh = H - step * 10
            subprocess.run(["i3-msg", "-s", i3sock, "resize", "set", str(rw), str(rh)], env=denv, check=False, timeout=5)
            time.sleep(0.06)
        subprocess.run(["i3-msg", "-s", i3sock, "floating", "disable"], env=denv, check=False, timeout=5)
        time.sleep(1.5)
        if "solidcraft:pane" in rects():
            vx = int(rects()["solidcraft:pane"]["x"]) - 2
            vy = int(H * 0.5)
            subprocess.run(["python3", XIN, "drag", str(vx), str(vy), str(vx - 80), str(vy)], env=denv, check=False, timeout=10)
            time.sleep(0.8)
            subprocess.run(["python3", XIN, "drag", str(vx - 80), str(vy), str(vx), str(vy)], env=denv, check=False, timeout=10)
        mark_phase("resize_end")
        time.sleep(8.0)
        mark_phase("post_resize_settle")
        time.sleep(2.5)
        pane_r = rects().get("solidcraft:pane", {})
        cur_wins = [w for w in x11_solidcraft_windows() if w["viewable"]]
        align_ok = False
        shape_info = []
        if pane_r and cur_wins and all(k in cur_wins[0]["geo"] for k in ("x", "y", "w", "h")):
            g = cur_wins[0]["geo"]
            rect_ok = (
                abs(g["x"] - pane_r["x"]) <= 2
                and abs(g["y"] - pane_r["y"]) <= 2
                and abs(g["w"] - pane_r["w"]) <= 2
                and abs(g["h"] - pane_r["h"]) <= 2
            )
            cids = [cur_wins[0]["wid"]]
            ch = subprocess.run(["xwininfo", "-children", "-id", cur_wins[0]["wid"]], env=denv, capture_output=True, text=True, timeout=5).stdout
            m_par = re.search(r"Parent window id:\s*(0x[0-9a-fA-F]+)", ch)
            if m_par:
                cids.append(m_par.group(1))
            shape_ok = True
            for cid in cids:
                sh = subprocess.run(["xwininfo", "-shape", "-id", cid], env=denv, capture_output=True, text=True, timeout=5).stdout
                shape_info.append(f"{cid}:{' '.join(sh.split())[:120]}")
                # If a bounding shape exists, it must not stay clipped smaller than the pane rect after resize settles
                if "No window shape" not in sh and "No bounding shape" not in sh:
                    dims = re.findall(r"(\d+)x(\d+)\+-?\d+\+-?\d+", sh)
                    if dims:
                        sw, sh_h = int(dims[0][0]), int(dims[0][1])
                        if sw < pane_r["w"] - 2 or sh_h < pane_r["h"] - 2:
                            shape_ok = False
            align_ok = rect_ok and shape_ok
        log("resize_rect_alignment", align_ok, f"pane={pane_r} x11={cur_wins[0]['geo'] if cur_wins else {}} shape={shape_info}")

        # 6. Pop out (detach) and Bring back (reattach) via pane header buttons, polling X11 viewable continuity
        def poll_viewable(dur_s):
            samples = []
            deadline = time.time() + dur_s
            while time.time() < deadline:
                samples.append(any(w["viewable"] for w in x11_solidcraft_windows()))
                time.sleep(0.05)
            return samples

        r_now = latest_probes()
        detach_rect = next((k for k in ("solidcraft:detach", "pane-tool:mech:solidcraft-pop") if k in r_now), None)
        if detach_rect and click_rect(detach_rect):
            mark_phase("detached")
            detach_samples = poll_viewable(1.5)
            time.sleep(1.0)
            key_shot("key_detached")
            p_detached = latest_probes()
            pop_wins = [w for w in x11_solidcraft_windows() if w["viewable"]]
            pop_geo = pop_wins[0]["geo"] if pop_wins else {}
            did_popout = (
                "solidcraft:bring_back" in p_detached
                and "solidcraft:detach" not in p_detached
                and pop_geo.get("w", 0) > pane_r.get("w", 0) + 100
            )
            # Click Bring back on SolidCraftWindow's own top-right header bar directly above the child window
            bring_back_clicked = False
            if all(k in pop_geo for k in ("x", "y", "w")):
                bx = int(pop_geo["x"] + pop_geo["w"] - 48)
                by = int(max(8, pop_geo["y"] - 16))
                subprocess.run(["python3", XIN, "click", str(bx), str(by)], env=denv, check=False, timeout=10)
                bring_back_clicked = True
            mark_phase("reattached")
            reattach_samples = poll_viewable(1.5)
            time.sleep(1.0)
            key_shot("key_reattached")
            p_reattached = latest_probes()
            re_wins = [w for w in x11_solidcraft_windows() if w["viewable"]]
            re_geo = re_wins[0]["geo"] if re_wins else {}
            unmapped_detach = detach_samples.count(False)
            unmapped_reattach = reattach_samples.count(False)
            reattached_ok = (
                did_popout
                and bring_back_clicked
                and "solidcraft:detach" in p_reattached
                and "solidcraft:bring_back" not in p_reattached
                and abs(re_geo.get("w", -1) - pane_r.get("w", 0)) <= 2
                and unmapped_detach == 0
                and unmapped_reattach == 0
            )
            log(
                "solidcraft_detach_reattach",
                reattached_ok,
                f"popout={did_popout} pop_w={pop_geo.get('w')} re_w={re_geo.get('w')} unmapped=(detach:{unmapped_detach}/{len(detach_samples)}, reattach:{unmapped_reattach}/{len(reattach_samples)})",
            )

        # Stop recording cleanly so mp4 finalizes
        try:
            os.killpg(rec.pid, signal.SIGINT)
            rec.wait(timeout=15)
        except Exception:
            pass
    finally:
        cleanup()

    # Extract frames from video.mp4 and compute anti-flicker / resize / flash metrics
    if os.path.isfile(P("video.mp4")):
        subprocess.run(["ffmpeg", "-v", "error", "-i", P("video.mp4"), P("frames/f_%05d.png")], check=False, timeout=120)
    frames = sorted(glob.glob(P("frames/f_*.png")))
    analyze_frames(frames, phase_marks, keyframes_dir=P("keyframes"), log=log, pane_rect=pane_r or None)

    report = {
        "video": P("video.mp4"),
        "frame_count": len(frames),
        "phase_marks": phase_marks,
        "checks": checks,
    }
    with open(P("report.json"), "w") as fh:
        json.dump(report, fh, indent=2)

    sys.exit(0 if checks and all(c["ok"] for c in checks) else 1)


def analyze_frames(frames, phase_marks, keyframes_dir, log, pane_rect=None):
    """Compute post-map flashes, 45x45 black-block blinks, visible-span black frames, doubled chrome, idle diffs, and resize_contact.png."""
    viewer_box = (int(W * 0.56), int(H * 0.12), int(W * 0.94), int(H * 0.92))

    def pane_img(idx):
        return Image.open(frames[idx]).convert("L").crop(viewer_box)

    def small_img(idx):
        return Image.open(frames[idx]).convert("L").resize((360, 225), Image.Resampling.BILINEAR)

    def frame_diff(a, b):
        return ImageStat.Stat(ImageChops.difference(a, b)).mean[0]

    # Auto-detect first mapped UI frame (skips pre-map Xvfb root black frames 1..~10 cleanly)
    first_map_f = next((i for i in range(len(frames)) if ImageStat.Stat(small_img(i)).mean[0] > 5.0), 0)

    # Honest post-startup black-frame check across every SolidCraft-visible span
    black_frames = []
    checked_spans = []
    for s_key, off_s, e_key, def_dur in [
        ("solidcraft_open", 2.0, "kicad_tab", 3.5),
        ("solidcraft_return", 1.0, "resize_start", 2.5),
        ("post_resize_settle", 0.0, "detached", 2.5),
    ]:
        if frames and s_key in phase_marks:
            sf = min(len(frames) - 1, max(first_map_f, int((phase_marks[s_key] + off_s) * FPS)))
            ef = min(len(frames), int((phase_marks.get(e_key, phase_marks[s_key] + def_dur)) * FPS))
            if ef > sf:
                checked_spans.append((sf, ef))
                for i in range(sf, ef):
                    st = ImageStat.Stat(pane_img(i))
                    if st.mean[0] < 4.0 and st.stddev[0] < 2.0:
                        black_frames.append(i)
    log("zero_black_frames", bool(checked_spans) and not black_frames, f"first_map_f={first_map_f} checked_spans={checked_spans} total_frames={len(frames)} black_frames={black_frames[:10]}")

    # Full post-map checks matching fable/fable-2 criteria: relative neighbour flash + 180x180 block blink + detach/reattach dark blink
    flashes = []
    block_flashes = []
    detach_dark_blinks = []
    detach_start_f = int(phase_marks["detached"] * FPS) if "detached" in phase_marks else len(frames)
    if len(frames) - first_map_f > 3:
        prev = small_img(first_map_f)
        cur = small_img(first_map_f + 1)
        def blk_dark(im, x, y):
            return sum(im.crop((x, y, x + 45, y + 45)).histogram()[:8]) / 2025.0
        for i in range(first_map_f + 1, len(frames) - 1):
            nxt = small_img(i + 1)
            d1 = frame_diff(prev, cur)
            d2 = frame_diff(cur, nxt)
            dn = frame_diff(prev, nxt)
            if d1 > 3.0 and d2 > 3.0 and dn < 0.4 * min(d1, d2):
                flashes.append((i + 1, round(d1, 2), round(d2, 2), round(dn, 2)))
            if sum(cur.histogram()[:8]) / 81000.0 >= 0.45 and i >= detach_start_f:
                detach_dark_blinks.append(i + 1)
            # Only scan 45x45 blocks if frame changed noticeably from either neighbour
            if d1 > 2.0 or d2 > 2.0:
                for y in range(0, 225, 45):
                    for x in range(0, 360, 45):
                        if blk_dark(cur, x, y) >= 0.90 and blk_dark(prev, x, y) < 0.30 and blk_dark(nxt, x, y) < 0.30:
                            block_flashes.append((i + 1, x * 4, y * 4))
                            break
            prev, cur = cur, nxt
        log("zero_single_frame_flashes", not flashes, f"checked {first_map_f+1}..{len(frames)-1} flashes={flashes[:10]}")
        log("zero_black_block_flashes", not block_flashes and not detach_dark_blinks, f"block_flashes={block_flashes[:10]} detach_dark_blinks={detach_dark_blinks[:10]}")

    # Generate 6-panel resize_contact.png: 3 highest-diff shrink frames + 3 highest-diff grow frames inside [resize_start..resize_end]
    if frames and "resize_start" in phase_marks and "resize_end" in phase_marks:
        r_start = max(1, int(phase_marks["resize_start"] * FPS))
        r_end = min(len(frames) - 1, int(phase_marks["resize_end"] * FPS))
        if r_end - r_start >= 6:
            mid = (r_start + r_end) // 2
            def top3(lo, hi):
                scored = sorted(((frame_diff(small_img(i - 1), small_img(i)), i) for i in range(lo, hi)), reverse=True)
                picked = []
                for _, idx in scored:
                    if all(abs(idx - p) >= 2 for p in picked):
                        picked.append(idx)
                        if len(picked) == 3:
                            break
                return sorted(picked) if len(picked) == 3 else [lo + (hi - lo) * k // 3 for k in range(3)]
            sample_idx = top3(r_start + 1, mid) + top3(mid, r_end)
            tw, th = 480, 300
            sheet = Image.new("RGB", (tw * 3, th * 2))
            for idx_pos, f_i in enumerate(sample_idx):
                im = Image.open(frames[f_i]).convert("RGB").resize((tw, th), Image.Resampling.BILINEAR)
                sheet.paste(im, ((idx_pos % 3) * tw, (idx_pos // 3) * th))
            os.makedirs(keyframes_dir, exist_ok=True)
            sheet.save(os.path.join(keyframes_dir, "resize_contact.png"))

            # Doubled-chrome NCC check across resize_start..resize_end (e.g. sidebar-collapse reflow ghosting)
            try:
                import numpy as np
                pr = pane_rect or {"x": 755, "y": 70, "w": 611, "h": 799}
                px, py, pw = int(pr["x"]), int(pr["y"]), int(pr["w"])
                ref_arr = np.asarray(Image.open(frames[max(0, r_start - 3)]).convert("L"), dtype=np.float32)
                templates = [
                    ("gizmo", px + pw - 115, py + 145, 85, 65),
                    ("search_cmd", px + pw - 165, py + 52, 150, 26),
                    ("sketch_btn", px + 115, py + 95, 72, 60),
                ]
                doubled_hits = []
                frame_arrays = {i: np.asarray(Image.open(frames[i]).convert("L"), dtype=np.float32) for i in range(r_start, r_end)}
                blank_resize = [
                    i + 1
                    for i, full_a in frame_arrays.items()
                    if float(full_a[py + 80:py + 580, px + 5:px + 265].std()) < 3.0
                ]
                log("zero_blank_resize_frames", not blank_resize, f"checked {r_start}..{r_end} blank={blank_resize[:15]}")
                for tname, tx, ty, tw, th in templates:
                    if tx < 0 or ty < 0 or ty + th > H or tx + tw > W:
                        continue
                    tpl = ref_arr[ty:ty + th, tx:tx + tw]
                    t0 = tpl - tpl.mean()
                    tn = float(np.sqrt((t0 ** 2).sum()))
                    if tn < 50.0:
                        continue
                    y0, y1 = max(0, ty - 15), min(H, ty + th + 15)
                    bh, bw = y1 - y0, W
                    oh, ow = bh - th + 1, bw - tw + 1
                    t_fft = np.fft.rfft2(t0[::-1, ::-1], s=(bh, bw))
                    o_fft = np.fft.rfft2(np.ones_like(t0), s=(bh, bw))
                    for i, full_a in frame_arrays.items():
                        band = full_a[y0:y1, :]
                        f_fft = np.fft.rfft2(band, s=(bh, bw))
                        num = np.fft.irfft2(f_fft * t_fft, s=(bh, bw))[th - 1:th - 1 + oh, tw - 1:tw - 1 + ow]
                        s1 = np.fft.irfft2(f_fft * o_fft, s=(bh, bw))[th - 1:th - 1 + oh, tw - 1:tw - 1 + ow]
                        s2 = np.fft.irfft2(np.fft.rfft2(band * band, s=(bh, bw)) * o_fft, s=(bh, bw))[th - 1:th - 1 + oh, tw - 1:tw - 1 + ow]
                        var = np.maximum(s2 - s1 * s1 / t0.size, 0.0)
                        ncc = np.where(var > 0.25 * tn * tn, num / (np.sqrt(np.maximum(var, 1e-6)) * tn), 0.0)
                        peaks = []
                        while True:
                            py_i, px_i = np.unravel_index(np.argmax(ncc), ncc.shape)
                            if ncc[py_i, px_i] < 0.78:
                                break
                            peaks.append((int(px_i), int(py_i + y0), round(float(ncc[py_i, px_i]), 2)))
                            ncc[max(0, py_i - 20):py_i + 20, max(0, px_i - 40):px_i + 40] = -1.0
                            if len(peaks) > 4:
                                break
                        if len(peaks) > 1:
                            doubled_hits.append((tname, i + 1, peaks))
                log("zero_doubled_chrome", not doubled_hits, f"checked {r_start}..{r_end} hits={doubled_hits[:10]}")
            except ImportError:
                pass

    oscillations = []
    if frames and "post_resize_settle" in phase_marks:
        s_f = min(len(frames) - 1, int((phase_marks["post_resize_settle"] + 0.5) * FPS))
        e_f = min(len(frames) - 1, s_f + int(1.5 * FPS))
        for i in range(s_f, e_f):
            d = frame_diff(pane_img(i), pane_img(i + 1))
            if d > 0.8:
                oscillations.append((i, round(d, 3)))
    log("zero_idle_flicker", len(frames) > 0 and not oscillations, f"post-settle frame diffs >0.8: {oscillations[:10]}")


if __name__ == "__main__":
    main()
