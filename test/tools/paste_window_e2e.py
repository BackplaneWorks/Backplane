#!/usr/bin/env python3
"""Native paste through X11, end to end, on a fresh temp home (never a real log).
Build first, one heavy job at a time through the hub's bend lock:
  scripts/bp-lock.sh bend -- systemd-run --user --scope -p MemoryMax=12G -p CPUQuota=1200% -- \
    env BACKPLANE_JOBS=2 scripts/build-app.sh src/app/main.bend build/backplane
  cc -O2 test/tools/xclipboard.c -lX11 -o build/xclipboard
  cc -O2 test/tools/xpoke.c -lX11 -ldl -o build/xpoke
then: python3 test/tools/paste_window_e2e.py [build/backplane]
XPOKE / XCLIPBOARD override the helpers' paths, PASTE_DISPLAY the display
(default :92; an Xvfb is started on it, or /tmp/xvfb-root's when none is
on PATH). Children are this script's own and are stopped by PID.
Artifacts (snapshots, logs) remain in the printed temporary directory.
"""
import json
import os
from pathlib import Path
import random
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import zlib

repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix="bp-b2-paste-"))
print(f"artifacts {root}", flush=True)
(root / "home").mkdir()
(root / "user/kart").mkdir(parents=True)
(root / "project").mkdir()
(root / "bin").mkdir()
for provider in ("claude", "codex", "grok", "muse", "omp", "curl"):
    p = root / "bin" / provider
    p.write_text("#!/bin/sh\nexit 1\n")
    p.chmod(0o755)
# A local skills fixture keeps startup and the background refresh offline.
kbase = root / "user/.cache/backplane/kistack"
(kbase / "paste-fixture/skills/pcb").mkdir(parents=True)
(kbase / "current").write_text("paste-fixture\n")
(kbase / "paste-fixture/skills/pcb/SKILL.md").write_text("# Clipboard test fixture\n")
# an hour ahead, so the fixture's project sorts above Scratch (made at start)
now = int(time.time()) + 3600
changes = [
    {"$": "ProjectCreated", "id": "paste-project", "title": "Paste fixture", "root": str(root / "project"), "at": now},
    {"$": "ThreadCreated", "id": "paste-thread", "project": "paste-project", "title": "Clipboard fixture", "env": "local", "branch": "", "worktree": str(root / "project"), "provider": "claude", "model": "", "at": now},
    {"$": "SettingSet", "key": "onboard.done", "value": "1", "at": now},
    {"$": "SettingSet", "key": "onboard.uber", "value": "1", "at": now},
    {"$": "SettingSet", "key": "restart.continue", "value": "off", "at": now},
]
(root / "home/events.jsonl").write_text("".join(json.dumps(c) + "\n" for c in changes))

def png(path, w, h):
    rng = random.Random(2)
    raw = b"".join(b"\0" + rng.randbytes(w * 3) for _ in range(h))
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
png(root / "small.png", 16, 17)
png(root / "large.png", 400, 400)
(root / "text.txt").write_text("plain clipboard text")
assert (root / "large.png").stat().st_size > 256 * 1024
assert (root / "small.png").stat().st_size % 3 != 0  # exercise base64 padding
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
DISP = os.environ.get("PASTE_DISPLAY", ":92")
SOCK = Path("/tmp/.X11-unix/X" + DISP.lstrip(":"))
env = dict(os.environ, DISPLAY=DISP, WAYLAND_DISPLAY="", HOME=str(root / "user"),
           BACKPLANE_SNAP=str(root / "frame.ppm"), BACKPLANE_NO_UPDATE="1", BACKPLANE_PEERS="",
           PATH=str(root / "bin") + ":/usr/bin:/bin")
children = []
logs = []

def spawn(args, log):
    stream = (root / log).open("wb")
    logs.append(stream)
    child = subprocess.Popen(args, env=env, stdout=stream, stderr=stream)
    children.append(child)
    return child

def until(fn, seconds=30):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = fn()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError("timed out waiting for " + fn.__name__)

XPOKE = os.environ.get("XPOKE", str(repo / "build/xpoke"))
XCLIPBOARD = os.environ.get("XCLIPBOARD", str(repo / "build/xclipboard"))

def poke(*args):
    subprocess.run([XPOKE, *map(str, args)], env=env, check=True)

def snapshot(name):
    time.sleep(0.8)
    dest = root / (name + ".png")
    subprocess.run([sys.executable, str(repo / "scripts/ppm-to-png.py"), str(root / "frame.ppm"), str(dest)], check=True)
    print(f"snapshot {dest}", flush=True)

def pasted(filename, mode="", log="clipboard.log"):
    child = spawn([XCLIPBOARD, "image/png", str(root / filename), *([mode] if mode else [])], log)
    until(lambda: b"clipboard ready" in (root / log).read_bytes())
    before = set((root / "home/attachments/paste-thread").glob("*"))
    poke("ctrl", "v")
    def landed():
        files = set((root / "home/attachments/paste-thread").glob("*")) - before
        return next((p for p in files if p.read_bytes() == (root / filename).read_bytes()), None)
    path = until(landed)
    print(f"ok exact attachment {path} ({path.stat().st_size} bytes)", flush=True)
    if mode == "incr":
        until(lambda: b"INCR finished" in (root / log).read_bytes())
        print("ok INCR exercised", flush=True)
    snapshot(Path(filename).stem)
    child.terminate()
    child.wait(timeout=5)

try:
    if SOCK.exists():
        raise RuntimeError(DISP + " already in use; this fixture does not reuse another test's display")
    xvfb = shutil.which("Xvfb") or "/tmp/xvfb-root/usr/bin/Xvfb"
    spawn([xvfb, DISP, "-screen", "0", "1400x900x24", "-nolisten", "tcp"], "xvfb.log")
    until(lambda: SOCK.exists())
    binary = str((repo / (sys.argv[1] if len(sys.argv) > 1 else "build/backplane")).resolve())
    app = spawn([binary, "--home", str(root / "home"), "--port", str(port), "--no-tailscale", "--foreground"], "app.log")
    until(lambda: (root / "frame.ppm").exists(), 60)
    time.sleep(2)
    snapshot("initial")
    # The seeded thread's row (first project, first thread), then the composer.
    poke("click", 100, 143)
    time.sleep(1)
    snapshot("thread")
    poke("click", 600, 790)
    pasted("small.png", log="small-owner.log")
    pasted("large.png", "incr", "large-owner.log")
    # An owner refusing TARGETS must still deliver text into the composer.
    owner = spawn([XCLIPBOARD, "text/plain", str(root / "text.txt"), "legacy"], "text-owner.log")
    until(lambda: b"clipboard ready" in (root / "text-owner.log").read_bytes())
    poke("ctrl", "v")
    until(lambda: any("plain clipboard text" in p.read_text() for p in (root / "home/drafts").glob("*") if p.is_file()))
    print("ok TARGETS refusal falls back to UTF8_STRING", flush=True)
    snapshot("text")
    owner.terminate()
    owner.wait(timeout=5)
    owner = spawn([XCLIPBOARD, "text/plain", str(root / "text.txt"), "string"], "string-owner.log")
    until(lambda: b"clipboard ready" in (root / "string-owner.log").read_bytes())
    poke("ctrl", "v")
    until(lambda: b"requested STRING" in (root / "string-owner.log").read_bytes())
    print("ok UTF8_STRING refusal falls back to STRING", flush=True)
except Exception:
    if (root / "frame.ppm").exists():
        snapshot("failure")
    raise
finally:
    for child in reversed(children):
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
    for stream in logs:
        stream.close()
