#!/usr/bin/env python3
"""A sent message never comes back to the window's composer, end to end on a
fresh temp home (never a real log): type, Enter, ctrl+k to another thread,
back again; the same in a bot's thread; an unsent draft survives a reopen
and a restart; a bot's sent text an older build kept is forgotten when its
thread is opened.
Build first, one heavy job at a time through the hub's bend lock:
  scripts/bp-lock.sh bend -- systemd-run --user --scope -p MemoryMax=12G -p CPUQuota=1200% -- \
    env BACKPLANE_JOBS=2 scripts/build-app.sh src/app/main.bend build/backplane
  cc -O2 test/tools/xpoke.c -lX11 -ldl -o build/xpoke
then: python3 test/tools/draft_window_e2e.py [build/backplane]
XPOKE overrides the helper's path, DRAFT_DISPLAY the display (default :93;
an Xvfb is started on it, or /tmp/xvfb-root's when none is on PATH).
Children are this script's own and are stopped by PID. Snapshots and logs
stay in the printed temporary directory.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix="bp-draft-"))
print(f"artifacts {root}", flush=True)
home = root / "home"
drafts = home / "drafts"
(root / "user").mkdir()
(root / "project").mkdir()
(root / "bin").mkdir()
drafts.mkdir(parents=True)
for provider in ("claude", "codex", "grok", "muse", "omp", "curl"):
    p = root / "bin" / provider
    p.write_text("#!/bin/sh\nexit 1\n")
    p.chmod(0o755)
kbase = root / "user/.cache/backplane/kistack"
(kbase / "draft-fixture/skills/pcb").mkdir(parents=True)
(kbase / "current").write_text("draft-fixture\n")
(kbase / "draft-fixture/skills/pcb/SKILL.md").write_text("# Draft test fixture\n")
now = int(time.time()) + 3600
A, B, BOT = "tdraft-a", "tdraft-b", "tdraft-bot"
changes = [
    {"$": "ProjectCreated", "id": "draft-project", "title": "Draft fixture", "root": str(root / "project"), "at": now},
    {"$": "ThreadCreated", "id": A, "project": "draft-project", "title": "Alpha", "env": "local", "branch": "", "worktree": str(root / "project"), "provider": "claude", "model": "", "at": now},
    {"$": "ThreadCreated", "id": B, "project": "draft-project", "title": "Bravo", "env": "local", "branch": "", "worktree": str(root / "project"), "provider": "claude", "model": "", "at": now},
    {"$": "ThreadCreated", "id": BOT, "project": "", "title": "Kit", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
    {"$": "BotSet", "id": "bdraft", "name": "Kit", "thread": BOT, "look": 0, "persona": "", "at": now},
    {"$": "MessagePosted", "thread": BOT, "msg": "m-old", "role": "user", "text": "sent by an older build", "at": now},
    {"$": "SettingSet", "key": "onboard.done", "value": "1", "at": now},
    {"$": "SettingSet", "key": "onboard.uber", "value": "1", "at": now},
    {"$": "SettingSet", "key": "restart.continue", "value": "off", "at": now},
]
(home / "events.jsonl").write_text("".join(json.dumps(c) + "\n" for c in changes))
# what an older build left: the bot's sent text kept as its draft
(drafts / BOT).write_text("sent by an older build")
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
DISP = os.environ.get("DRAFT_DISPLAY", ":93")
SOCK = Path("/tmp/.X11-unix/X" + DISP.lstrip(":"))
env = dict(os.environ, DISPLAY=DISP, WAYLAND_DISPLAY="", HOME=str(root / "user"),
           BACKPLANE_SNAP=str(root / "frame.ppm"), BACKPLANE_NO_UPDATE="1", BACKPLANE_PEERS="",
           PATH=str(root / "bin") + ":/usr/bin:/bin")
children = []
logs = []
fails = []

def spawn(args, log):
    stream = (root / log).open("ab")
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
    return fn()

XPOKE = os.environ.get("XPOKE", str(repo / "build/xpoke"))

def poke(*args):
    subprocess.run([XPOKE, *map(str, args)], env=env, check=True)
    time.sleep(0.25)

def snapshot(name):
    time.sleep(0.8)
    dest = root / (name + ".png")
    subprocess.run([sys.executable, str(repo / "scripts/ppm-to-png.py"), str(root / "frame.ppm"), str(dest)], check=True)
    print(f"snapshot {dest}", flush=True)

def draft(t):
    p = drafts / t
    return p.read_text() if p.exists() else ""

def check(name, got, want):
    if got == want:
        print(f"ok {name}", flush=True)
    else:
        print(f"FAIL {name}: got {got!r} want {want!r}", flush=True)
        fails.append(name)

def posted(text):
    return any(json.loads(l).get("text") == text for l in (home / "events.jsonl").read_text().splitlines() if '"MessagePosted"' in l)

def go(title):
    poke("ctrl", "k")
    time.sleep(0.4)
    poke("type", title)
    time.sleep(0.4)
    poke("key", "Return")
    time.sleep(0.8)

def start():
    app = spawn([binary, "--home", str(home), "--port", str(port), "--no-tailscale", "--foreground"], "app.log")
    until(lambda: (root / "frame.ppm").exists(), 60)
    time.sleep(3)
    return app

def stop(app):
    app.terminate()
    app.wait(timeout=10)
    (root / "frame.ppm").unlink(missing_ok=True)

try:
    if SOCK.exists():
        raise RuntimeError(DISP + " already in use; this fixture does not reuse another test's display")
    xvfb = shutil.which("Xvfb") or "/tmp/xvfb-root/usr/bin/Xvfb"
    spawn([xvfb, DISP, "-screen", "0", "1400x900x24", "-nolisten", "tcp"], "xvfb.log")
    until(lambda: SOCK.exists())
    binary = str((repo / (sys.argv[1] if len(sys.argv) > 1 else "build/backplane")).resolve())
    app = start()
    snapshot("initial")
    # a thread: type, Enter, away, back
    go("Alpha")
    poke("type", "hello sent")
    check("typing is kept", until(lambda: draft(A) == "hello sent", 5) and draft(A), "hello sent")
    poke("key", "Return")
    check("sent", bool(until(lambda: posted("hello sent"), 10)), True)
    check("send forgets the kept copy", until(lambda: draft(A) == "", 5) and draft(A), "")
    snapshot("a-sent")
    go("Bravo")
    poke("type", "bravo unsent")
    check("unsent kept", until(lambda: draft(B) == "bravo unsent", 5) and draft(B), "bravo unsent")
    snapshot("b-typed")
    go("Alpha")
    snapshot("a-reopened")
    check("reopen writes nothing back", draft(A), "")
    go("Bravo")
    snapshot("b-reopened")
    # the bot's thread: the old kept text is forgotten on open, then a send
    go("Kit")
    check("old kept sent text forgotten", until(lambda: draft(BOT) == "", 5) and draft(BOT), "")
    snapshot("bot-opened")
    poke("type", "hi kit")
    check("bot typing kept", until(lambda: draft(BOT) == "hi kit", 5) and draft(BOT), "hi kit")
    poke("key", "Return")
    check("bot sent", bool(until(lambda: posted("hi kit"), 10)), True)
    check("bot send forgets the kept copy", until(lambda: draft(BOT) == "", 5) and draft(BOT), "")
    go("Alpha")
    go("Kit")
    snapshot("bot-reopened")
    check("bot reopen writes nothing back", draft(BOT), "")
    # a restart: only the unsent draft comes back
    stop(app)
    app = start()
    go("Alpha")
    snapshot("restart-a")
    go("Bravo")
    snapshot("restart-b")
    go("Kit")
    snapshot("restart-bot")
    check("after restart: drafts on disk", sorted(p.name for p in drafts.iterdir() if p.is_file()), [B])
    check("after restart: unsent survives", draft(B), "bravo unsent")
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
sys.exit(1 if fails else 0)
