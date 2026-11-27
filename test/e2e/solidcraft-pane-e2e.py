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
        "/data/bp-tmp/sc-fable-dl/x/solidcraft-0.50.0-linux-x86_64/bin",
        os.path.dirname(shutil.which("solidcraft") or ""),
    ]:
        if cand and os.path.isfile(os.path.join(cand, "solidcraft")) and os.path.isfile(os.path.join(cand, "solidcraft-cli")):
            return cand
    return ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("client_bin")
    ap.add_argument("hub_bin")
    ap.add_argument("assets_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--step", default="/home/h/Documents/Nordic/DK_Cases/out/nrf54lm20dk_case.step")
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
                    "steps": [
                        {"command": "parameters.define", "params": {"name": "w", "expression": "40 mm"}},
                        {"command": "sketch.create", "params": {"plane": "xy", "name": "base"}},
                        {"command": "sketch.rectangle", "params": {"center": [0, 0], "width": "w", "height": "25 mm"}},
                        {"command": "solid.extrude", "params": {"distance": "12 mm"}},
                    ]
                },
                fh,
            )
        subprocess.run([cli, "run", script_path, "--save", P("proj/mech/box.sc"), "--out", P("proj/mech/box.stl")], check=False)

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

    # MCP call list for Project File Tools + viewer_show + viewer_render
    mcp_calls = [
        {"name": "files_list", "arguments": {"path": ""}},
        {"name": "files_write", "arguments": {"path": "mech/spec.txt", "text": "housing v1\n"}},
        {"name": "files_read", "arguments": {"path": "mech/spec.txt"}},
        {"name": "files_read", "arguments": {"path": "mech/box.sc"}},
        {"name": "files_show", "arguments": {"path": "mech"}},
        {"name": "viewer_render", "arguments": {"path": "mech/case.step", "view": "iso"}},
        {"name": "viewer_show", "arguments": {"kind": "mech", "path": "mech/case.step"}},
    ]
    with open(P("mcp_calls.json"), "w") as fh:
        json.dump(mcp_calls, fh)

    procs = []
    checks = []

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
            fh.write(f"font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket {RUN}/i3.sock\n")
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
            {"view_show": ["mech", P("proj/mech/case.step")]},
            {"wait": 400},
            {"solidcraft": {"report": True, "tag": "while_starting"}},
            {"solidcraft": {"open": P("proj/mech/box.sc"), "tag": "open_while_starting"}},
            {"solidcraft": {"wait_ready": 45000, "min_painted": 5, "tag": "ready_after_race"}},
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
        for _ in range(90):
            ms = [json.loads(l.split("GPUI_METRIC ", 1)[1]) for l in open(P("client.log"), errors="replace") if "GPUI_METRIC " in l] if os.path.isfile(P("client.log")) else []
            if any(m.get("event") == "solidcraft" and m.get("tag") == "ready_after_race" for m in ms):
                break
            time.sleep(0.5)

        sc_events = [json.loads(l.split("GPUI_METRIC ", 1)[1]) for l in open(P("client.log"), errors="replace") if "GPUI_METRIC " in l]
        ev_start = next((m for m in sc_events if m.get("event") == "solidcraft" and m.get("tag") == "while_starting"), {})
        ev_ready = next((m for m in sc_events if m.get("event") == "solidcraft" and m.get("tag") == "ready_after_race"), {})
        same_proc = (
            ev_ready.get("state") == "ready"
            and ev_start.get("pid", 0) != 0
            and ev_start.get("pid") == ev_ready.get("pid")
            and ev_start.get("gen") == ev_ready.get("gen")
            and ev_ready.get("file", "").endswith("box.sc")
        )
        log("open_while_starting_no_respawn", same_proc, f"start={ev_start.get('pid')}/{ev_start.get('gen')} ready={ev_ready.get('pid')}/{ev_ready.get('gen')} file={ev_ready.get('file')}")

        def metrics():
            if not os.path.isfile(P("client.log")):
                return []
            return [json.loads(l.split("GPUI_METRIC ", 1)[1]) for l in open(P("client.log"), errors="replace") if "GPUI_METRIC " in l]

        def rects():
            r = {}
            for m in metrics():
                if m.get("event") == "layout":
                    r.update(m.get("rects", {}))
            return r

        def ctrls():
            c = {}
            for m in metrics():
                if m.get("event") == "controls":
                    for item in m.get("items", []):
                        c[item.get("id", "")] = item
            return c

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
                    hits.append({"wid": wid, "line": line.strip(), "viewable": "IsViewable" in info, "info": info})
            return hits

        phase_marks = {}
        def mark_phase(name):
            phase_marks[name] = time.time() - t_rec

        # 1. Trigger stand-in turn (`zqcalls`) to exercise project file tools + viewer_render + viewer_show
        subprocess.run(["python3", XIN, "focus", "Backplane"], env=denv, check=False, timeout=10)
        if click_rect("composer"):
            subprocess.run(["python3", XIN, "type", "zqcalls"], env=denv, check=False, timeout=15)
            subprocess.run(["python3", XIN, "key", "Return"], env=denv, check=False, timeout=10)
        time.sleep(6.0)
        fake_log = open(P("fake.log"), errors="replace").read() if os.path.isfile(P("fake.log")) else ""
        log("file_tools_mcp", "CALL files_list ok" in fake_log and "CALL files_write ok" in fake_log and "CALL files_read error" in fake_log,
            "stand-in MCP file tools logged: " + "; ".join([l for l in fake_log.splitlines() if "CALL " in l])[:300])
        log("viewer_render_cli", "CALL viewer_render ok" in fake_log, "viewer_render result in fake.log")

        # 2. Switch to Files tab and click a CAD file, then verify SolidCraft embeds
        mark_phase("files_tab")
        click_rect("rail:files")
        time.sleep(1.5)
        key_shot("key_files")

        mark_phase("solidcraft_open")
        click_rect("rail:mech")
        time.sleep(4.0)
        key_shot("key_step_open")
        sc_wins = x11_solidcraft_windows()
        log("solidcraft_embedded", any(w["viewable"] for w in sc_wins), f"SolidCraft X11 windows: {[w['line'] for w in sc_wins]}")

        # 3. Switch across KiCad Board -> Files -> SolidCraft Mech and verify zero Z-order bleed
        mark_phase("kicad_tab")
        click_rect("rail:board")
        time.sleep(2.0)
        key_shot("key_kicad_tab")
        sc_hidden_on_board = not any(w["viewable"] for w in x11_solidcraft_windows())
        log("no_zorder_bleed_board", sc_hidden_on_board, f"SolidCraft hidden on Board tab: {sc_hidden_on_board}")

        mark_phase("solidcraft_return")
        click_rect("rail:mech")
        time.sleep(2.5)
        key_shot("key_solidcraft_return")

        # 4. Resize GPUI window via i3-msg and restore
        mark_phase("resize")
        i3sock = f"{RUN}/i3.sock"
        subprocess.run(["i3-msg", "-s", i3sock, "floating", "enable"], env=denv, check=False, timeout=5)
        subprocess.run(["i3-msg", "-s", i3sock, "resize", "set", "1100", "720"], env=denv, check=False, timeout=5)
        time.sleep(1.5)
        key_shot("key_resized")
        subprocess.run(["i3-msg", "-s", i3sock, "floating", "disable"], env=denv, check=False, timeout=5)
        time.sleep(1.5)
        mark_phase("post_resize_settle")
        time.sleep(2.0)

        # 5. Detach and reattach if controls exist
        c = ctrls()
        detach_id = next((k for k in c if "detach" in k or "popout" in k), None)
        if detach_id and click_rect(detach_id):
            mark_phase("detached")
            time.sleep(2.0)
            key_shot("key_detached")
            c2 = ctrls()
            attach_id = next((k for k in c2 if "bring" in k or "popin" in k or "attach" in k), None)
            if attach_id and click_rect(attach_id):
                mark_phase("reattached")
                time.sleep(2.0)
                key_shot("key_reattached")

        # Stop recording cleanly so mp4 finalizes
        try:
            os.killpg(rec.pid, signal.SIGINT)
            rec.wait(timeout=15)
        except Exception:
            pass
    finally:
        cleanup()

    # Extract frames from video.mp4 and compute anti-flicker metrics
    if os.path.isfile(P("video.mp4")):
        subprocess.run(["ffmpeg", "-v", "error", "-i", P("video.mp4"), P("frames/f_%05d.png")], check=False, timeout=120)
    frames = sorted(glob.glob(P("frames/f_*.png")))
    viewer_box = (int(W * 0.48), int(H * 0.12), int(W * 0.96), int(H * 0.92))

    def pane_img(idx):
        return Image.open(frames[idx]).convert("L").crop(viewer_box)

    def frame_diff(a, b):
        return ImageStat.Stat(ImageChops.difference(a, b)).mean[0]

    black_frames = []
    if frames and "solidcraft_open" in phase_marks:
        start_f = min(len(frames) - 1, int((phase_marks["solidcraft_open"] + 2.0) * FPS))
        end_f = min(len(frames), int((phase_marks.get("kicad_tab", phase_marks["solidcraft_open"] + 4.0)) * FPS))
        for i in range(start_f, end_f):
            st = ImageStat.Stat(pane_img(i))
            if st.mean[0] < 4.0 and st.stddev[0] < 2.0:
                black_frames.append(i)
    log("zero_black_frames", len(frames) > 0 and not black_frames, f"checked frames={len(frames)}, black_frames={black_frames[:10]}")

    oscillations = []
    if frames and "post_resize_settle" in phase_marks:
        s_f = min(len(frames) - 1, int((phase_marks["post_resize_settle"] + 0.5) * FPS))
        e_f = min(len(frames) - 1, s_f + int(1.5 * FPS))
        for i in range(s_f, e_f):
            d = frame_diff(pane_img(i), pane_img(i + 1))
            if d > 0.8:
                oscillations.append((i, round(d, 3)))
    log("zero_idle_flicker", len(frames) > 0 and not oscillations, f"post-settle frame diffs >0.8: {oscillations[:10]}")

    report = {
        "video": P("video.mp4"),
        "frame_count": len(frames),
        "phase_marks": phase_marks,
        "checks": checks,
    }
    with open(P("report.json"), "w") as fh:
        json.dump(report, fh, indent=2)

    sys.exit(0 if checks and all(c["ok"] for c in checks) else 1)


if __name__ == "__main__":
    main()
