#!/usr/bin/env python3
"""A temp hub for the phones' swarm screens (docs/SWARM.md): stand-in agents, a project "toy" with two threads and two live swarms (one paused), and a
board with every kind of post (idea, critique, question, a claim waiting on a peer's lock, a done, a guard post, a held post, a synthesis-free run so far).

    python3 -I test/swarm/rig.py up HUB_BIN DIR       starts it in the background; DIR/env has PORT, HOST, TOKEN, HOME, PID, LINK
    bun test/swarm/probe.ts HOST PORT TOKEN 'swarm.list {}' ...   data/1 calls against it
    python3 -I test/swarm/rig.py down DIR

HOST (default 127.0.0.1) is where the hub listens: a simulator on another Mac needs the tailnet address. The member threads run on the claude stand-in
(test/tools/claude_standin.ts through bun), so a post from the phone, an added member or a new swarm wakes or starts them and nothing real runs."""
import json, os, shutil, signal, socket, subprocess, sys, time, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
def env_of(d): return dict(l.split("=", 1) for l in open(f"{d}/env").read().split("\n") if "=" in l)

def seed(d, now):
    toy = f"{d}/toy"; os.makedirs(f"{toy}/src", exist_ok=True)
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": now} for k, v in [("restart.continue", "off"), ("onboard.done", "1"), ("onboard.uber", "1"), ("person.tz", "UTC")]]
    ev.append({"$": "ProjectCreated", "id": "p1", "title": "toy", "root": toy, "at": now - 4000})
    def thread(id, title, provider, model, at):
        ev.append({"$": "ThreadCreated", "id": id, "project": "p1", "title": title, "env": "local", "branch": "", "worktree": "", "provider": provider, "model": model, "at": at})
    thread("t-enc", "Encoder driver", "claude", "opus", now - 3000)
    thread("t-notes", "Datasheet notes", "claude", "sonnet", now - 2500)
    members = [("t-opus", "opus", "#3b82f6", "", "claude", "opus"), ("t-gem", "gemini", "#f97316", "", "claude", "sonnet"), ("t-codex", "critic", "#10b981", "critic", "claude", "haiku")]
    for th, name, _, _, prov, model in members: thread(th, f"debounce · {name}", prov, model, now - 2000)
    room = "rsw1"
    ev.append({"$": "RoomSet", "id": room, "name": "debounce", "members": "\n".join("thread:" + m[0] for m in members), "ephemeral": False, "closed": False,
               "kind": "swarm", "project": "p1", "goal": "Pick a debounce for the rotary encoder and land it with tests.", "by": "", "state": "active", "reason": "", "at": now - 2000})
    for th, name, color, role, prov, model in members:
        ev.append({"$": "SettingSet", "key": f"room.member.{room}.thread:{th}", "value": json.dumps({"name": name, "color": color, "role": role, "provider": prov, "model": model}), "at": now - 2000})
    posts = [
        ("sp1", "you", "", "The encoder bounces for about 2 ms on every detent. Pick a debounce and land it.", {}, 1900),
        ("sp2", "opus", "t-opus", "Idea: a 5 ms integrator on the A and B lines, sampled at 1 kHz.", {"tags": ["idea"]}, 1800),
        ("sp3", "critic", "t-codex", "5 ms adds lag at fast spins. A state machine on the quadrature code rejects bounce without a delay.", {"tags": ["critique"], "reply": "sp2"}, 1700),
        ("sp4", "gemini", "t-gem", "@opus does the datasheet give a worst-case bounce time?", {"tags": ["question"], "mentions": ["thread:t-opus"]}, 1600),
        ("sp5", "opus", "t-opus", "3 ms worst case. Building on critic's state machine.", {"reply": "sp4"}, 1500),
        ("sp6", "gemini", "t-gem", "I'll write the state machine.", {"tags": ["claim"], "files": ["src/encoder.rs"]}, 1400),
        ("sp7", "critic", "t-codex", "I'll add the tests next to it.", {"tags": ["claim"], "files": ["src/encoder.rs", "tests/encoder.rs"]}, 1300),
        ("sp8", "hub", "", "critic waits for src/encoder.rs: gemini holds it.", {"guard": True}, 1290),
        ("sp9", "opus", "t-opus", "Bench numbers: 0 missed steps at 1200 rpm with the state machine.", {"tags": ["done"]}, 1100),
        ("sp10", "critic", "t-codex", "again: the state machine rejects bounce without a delay", {"held": "circular"}, 1000),
    ]
    for pid, frm, th, text, extra, ago in posts:
        e = {"$": "RoomPosted", "id": pid, "room": room, "from": frm, "text": text, "hop": 0, "at": now - ago}
        if th: e["thread"] = th
        e.update(extra); ev.append(e)
    ev.append({"$": "SettingSet", "key": f"room.claim.{room}.sp6", "value": json.dumps({"thread": "t-gem", "files": ["src/encoder.rs"], "locks": [], "state": "held"}), "at": now - 1400})
    ev.append({"$": "SettingSet", "key": f"room.claim.{room}.sp7", "value": json.dumps({"thread": "t-codex", "files": ["src/encoder.rs", "tests/encoder.rs"], "locks": [], "state": "waiting"}), "at": now - 1300})
    # a second, paused swarm, and a finished one the list leaves out
    thread("t-a", "enclosure · fit", "claude", "opus", now - 900); thread("t-b", "enclosure · print", "claude", "sonnet", now - 900)
    ev.append({"$": "RoomSet", "id": "rsw2", "name": "enclosure fit", "members": "thread:t-a\nthread:t-b", "ephemeral": False, "closed": False, "kind": "swarm", "project": "p1",
               "goal": "Fit the board in the printed shell.", "by": "", "state": "paused", "reason": "idle", "at": now - 900})
    thread("t-c", "old · one", "claude", "opus", now - 5000); thread("t-d", "old · two", "claude", "opus", now - 5000)
    ev.append({"$": "RoomSet", "id": "rsw0", "name": "old question", "members": "thread:t-c\nthread:t-d", "ephemeral": False, "closed": False, "kind": "swarm", "project": "p1",
               "goal": "", "by": "", "state": "done", "reason": "", "at": now - 5000})
    return ev

def up(hub, d):
    shutil.rmtree(d, ignore_errors=True)
    home, agents, fake = f"{d}/home", f"{d}/bin", f"{d}/fakehome"
    for p in (home, agents, fake): os.makedirs(p, exist_ok=True)
    bun = shutil.which("bun") or os.path.expanduser("~/.bun/bin/bun")
    open(f"{agents}/claude", "w").write(f"#!/bin/sh\nexec {bun} {ROOT}/test/tools/claude_standin.ts \"$@\"\n"); os.chmod(f"{agents}/claude", 0o755)
    for n in ["codex", "grok", "muse", "agy", "gemini", "opencode", "pi", "omp"]:
        open(f"{agents}/{n}", "w").write("#!/bin/sh\nexit 1\n"); os.chmod(f"{agents}/{n}", 0o755)
    now = int(time.time())
    open(f"{home}/events.jsonl", "w").write("".join(json.dumps(e) + "\n" for e in seed(d, now)))
    host = os.environ.get("HOST", "127.0.0.1"); port = int(os.environ.get("PORT", "0"))
    if not port: s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
    env = {k: v for k, v in os.environ.items() if not k.startswith("BACKPLANE_") and k not in ("DISPLAY", "WAYLAND_DISPLAY")}
    env.update(BACKPLANE_AGENTS=agents, BACKPLANE_NO_UPDATE="1", BACKPLANE_NAME="swarm-rig", BP_STANDIN_MODE="stream", HOME=fake, XDG_CACHE_HOME=f"{fake}/.cache",
               XDG_CONFIG_HOME=f"{fake}/.config", XDG_DATA_HOME=f"{fake}/.local", BACKPLANE_LOCK_DIR=d, LC_ALL="C")
    p = subprocess.Popen([hub, "--home", home, "--port", str(port), "--host", host, "--no-tailscale", "--foreground"], cwd=d, stdout=open(f"{d}/hub.log", "wb"),
                         stderr=subprocess.STDOUT, env=env, start_new_session=True)
    for _ in range(150):
        try: urllib.request.urlopen(f"http://{host}:{port}/healthz", timeout=2); break
        except Exception: time.sleep(0.2)
    tok = open(f"{home}/token").read().strip()
    open(f"{d}/env", "w").write(f"PORT={port}\nHOST={host}\nTOKEN={tok}\nHOME={home}\nPID={p.pid}\nLINK=http://{host}:{port}/#token={tok}\n")
    print(open(f"{d}/env").read().replace(tok, "<token>"), end="")

def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "up": up(sys.argv[2], sys.argv[3])
    elif cmd == "down":
        e = env_of(sys.argv[2])
        try: os.killpg(int(e["PID"]), signal.SIGTERM)
        except ProcessLookupError: pass
        print("down")
    else: sys.exit(__doc__)

main()
