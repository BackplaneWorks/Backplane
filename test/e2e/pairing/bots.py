#!/usr/bin/env python3
"""Bots across two linked hubs: two temp-home hubs (this script starts them, with the stand-in agent, --no-tailscale), linked the way link.py links them
(bots.invite on A, bots.join on B) and marked as the owner's own (bots.peer_own). Then:
  - a bot on A launches a thread on B (thread_launch with `machine`), B's stand-in agent answers, and the report comes back to the bot on A;
  - a room with a bot on each hub: the person's post on either hub reaches the other, and the room is in both logs;
  - plane_read of B from A's bot;
  - with the link no longer the owner's (bots.peer_own own=false on A), the same launch and a post from B reach nothing.

  bots.py --hub PATH/backplane-hub [--port-a 3821] [--port-b 3822]

Needs bun (the stand-in is test/tools/claude_standin.ts). Protocol result, not a client one. Exit 1 when a check failed.
"""
import argparse, json, os, shutil, subprocess, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, report, finish  # noqa: E402
from settings_revoke import View  # noqa: E402
from link import rpc  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../.."))


def log_of(home):
    out = []
    for line in open(os.path.join(home, "events.jsonl"), errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        out.append(e.get("e", e) if isinstance(e, dict) else e)
    return out


def until(f, seconds=45):
    end = time.time() + seconds
    while time.time() < end:
        v = f()
        if v:
            return v
        time.sleep(0.5)
    return None


def start(hub_bin, root, name, port, other_port, agents):
    home = os.path.join(root, name, "home")
    os.makedirs(home, exist_ok=True)
    proj = os.path.join(root, name, "proj")
    os.makedirs(proj, exist_ok=True)
    subprocess.run(["git", "init", "-q"], cwd=proj, check=True)
    if not os.path.exists(os.path.join(home, "events.jsonl")):
        with open(os.path.join(home, "events.jsonl"), "w") as f:
            f.write(json.dumps({"$": "ProjectCreated", "id": "p1", "title": "far-" + name, "root": proj, "at": 1}) + "\n")
    env = {"PATH": agents + ":/usr/bin:/bin", "HOME": os.path.join(root, name), "BACKPLANE_NO_UPDATE": "1", "BACKPLANE_AGENTS": agents,
           "BACKPLANE_ORDER_SERVER": "http://127.0.0.1:9", "BACKPLANE_KEYRING_LOCKED": "0", "BACKPLANE_PEERS": f"http://127.0.0.1:{other_port}",
           "FAKE_LOG": os.path.join(root, name, "standin.log"), "BP_STANDIN_MODE": "stream", "BP_STREAM_N": "2", "BP_STREAM_MS": "50"}
    p = subprocess.Popen([hub_bin, "--home", home, "--port", str(port), "--no-tailscale", "--foreground"], env=env,
                         stdout=open(os.path.join(root, name, "hub.log"), "w"), stderr=subprocess.STDOUT)
    return p, home


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--hub", required=True)
    a.add_argument("--port-a", type=int, default=3821)
    a.add_argument("--port-b", type=int, default=3822)
    o = a.parse_args()
    bun = shutil.which("bun")
    if not bun:
        report("bots across hubs", "not run", "bun (the stand-in agent's runtime) is not installed")
        return finish()
    base = os.path.join(os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")))
    os.makedirs(base, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-bots-", dir=base)
    agents = os.path.join(root, "bin")
    os.makedirs(agents)
    for n in ("claude",):
        with open(os.path.join(agents, n), "w") as f:
            f.write(f'#!/bin/sh\nexec {bun} {ROOT}/test/tools/claude_standin.ts "$@"\n')
        os.chmod(os.path.join(agents, n), 0o755)
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(agents, n), "w") as f:
            f.write("#!/bin/sh\nexit 1\n")
        os.chmod(os.path.join(agents, n), 0o755)
    pa, ha = start(o.hub, root, "a", o.port_a, o.port_b, agents)
    A, B = (Hub(f"http://127.0.0.1:{p}", "") for p in (o.port_a, o.port_b))
    A.wait_up(60)  # B looks for A when it starts: A must be listening by then
    pb, hb = start(o.hub, root, "b", o.port_b, o.port_a, agents)
    try:
        if not (A.wait_up(60) and B.wait_up(60)):
            check("both hubs up", False, open(os.path.join(root, "a", "hub.log")).read()[-300:])
            return finish()
        # a hub looks for its machines when it starts (and every two minutes): A started before B was up, so A starts again, now finding B
        pa.terminate(); pa.wait(15)
        pa, ha = start(o.hub, root, "a", o.port_a, o.port_b, agents)
        if not A.wait_up(60):
            check("A back up", False)
            return finish()
        time.sleep(3)
        A.root = open(os.path.join(ha, "token")).read().strip()
        B.root = open(os.path.join(hb, "token")).read().strip()
        inv = rpc(A, "bots.invite")
        j = rpc(B, "bots.join", {"invite": inv["invite"]})
        link = inv["peer"]
        check("linked", bool(j.get("peer")), json.dumps(j)[:200])
        time.sleep(1)
        # the link is marked the owner's own (what the pairing lane decides; here the test says so)
        for hub in (A, B):
            r = rpc(hub, "bots.peer_own", {"peer": link, "own": True})
            check("link marked as the owner's own", r.get("ok") is True, json.dumps(r)[:200])
        time.sleep(1)

        # bots: ada on A, bob on B
        ada = rpc(A, "bots.create", {"name": "ada"})["bot"]
        bob = rpc(B, "bots.create", {"name": "bob"})["bot"]
        th = lambda home, bot: next(e["thread"] for e in log_of(home) if e.get("$") == "BotSet" and e.get("id") == bot)
        tha, thb = th(ha, ada), th(hb, bob)
        until(lambda: any(e.get("$") == "WorktreeSet" and e.get("id") == tha for e in log_of(ha)))

        # 1. delegation: ada's agent (the stand-in) launches a thread on B and hears back
        call = {"name": "thread_launch", "arguments": {"machine": link, "project": "p1", "prompt": "mode:plain work on the other machine", "title": "far work"}}
        rpc(A, "bots.say", {"bot": ada, "text": "mcpcall:" + json.dumps(call)})
        launched = until(lambda: next((e for e in log_of(hb) if e.get("$") == "ThreadLaunched" and e.get("by") == f"{link}~{tha}"), None))
        check("B launched a thread for ada's thread", bool(launched), "no ThreadLaunched with by=<link>~<ada's thread> on B")
        report_back = until(lambda: next((e for e in log_of(ha) if e.get("$") == "MessagePosted" and e.get("thread") == tha and "Launched thread" in e.get("text", "")
                                           and "completed" in e.get("text", "")), None), 60)
        check("the report came back to ada on A when B's turn ended", bool(report_back), "no '[Backplane] Launched thread .. completed' message in ada's thread")
        if launched:
            far_id = f"{link}~{launched['id']}"
            upd = {"name": "thread_send", "arguments": {"threadId": far_id, "text": "one more thing"}}
            rpc(A, "bots.say", {"bot": ada, "text": "mcpcall:" + json.dumps(upd)})
            got = until(lambda: next((e for e in log_of(hb) if e.get("$") == "MessagePosted" and e.get("thread") == launched["id"] and e.get("text") == "one more thing"), None))
            check("thread_send reaches the thread on B by its <link>~<id>", bool(got))

        # 2. a room with a bot on each hub
        members = f"{ada}\nbob@{link}"
        r = rpc(A, "bots.room", {"room": "rfar", "name": "far lab", "members": members})
        check("A makes a room with bob@<link>", r.get("ok") is True, json.dumps(r)[:200])
        on_b = until(lambda: any(e.get("$") == "RoomSet" and e.get("id") == "rfar" for e in log_of(hb)))
        check("the room is on B", bool(on_b))
        rpc(A, "bots.post", {"room": "rfar", "text": "hello from A"})
        heard_b = until(lambda: any(e.get("$") == "RoomPosted" and e.get("room") == "rfar" and e.get("text") == "hello from A" for e in log_of(hb)))
        check("a post on A is in B's copy of the room", bool(heard_b))
        bob_told = until(lambda: any(e.get("$") == "MessagePosted" and e.get("thread") == thb and "hello from A" in e.get("text", "") for e in log_of(hb)))
        check("bob (a bot on B) heard it", bool(bob_told))
        rpc(B, "bots.post", {"room": "rfar", "text": "hello from B"})
        heard_a = until(lambda: any(e.get("$") == "RoomPosted" and e.get("room") == "rfar" and e.get("text") == "hello from B" for e in log_of(ha)))
        check("a post on B is in A's copy of the room", bool(heard_a))
        time.sleep(2)
        n_a = sum(1 for e in log_of(ha) if e.get("$") == "RoomPosted" and e.get("room") == "rfar")
        n_b = sum(1 for e in log_of(hb) if e.get("$") == "RoomPosted" and e.get("room") == "rfar")
        check("each post is once on each hub (nothing echoes)", (n_a, n_b) == (2, 2), f"A {n_a} B {n_b}")

        # 3. plane_read of B from ada
        rpc(A, "bots.say", {"bot": ada, "text": "mcpcall:" + json.dumps({"name": "plane_read", "arguments": {"machine": link}})})
        plane = until(lambda: next((e for e in log_of(ha) if e.get("$") == "MessagePosted" and e.get("thread") == tha and e.get("role") == "assistant" and "MCPCALL plane_read" in e.get("text", "")), None))
        check("plane_read with machine returns B's plane", bool(plane) and "Status plane" in plane.get("text", ""), (plane or {}).get("text", "")[:200])

        # 4. the link is no longer the owner's own on A: nothing crosses either way
        rpc(A, "bots.peer_own", {"peer": link, "own": False})
        time.sleep(1)
        before_b = len(log_of(hb))
        rpc(A, "bots.say", {"bot": ada, "text": "mcpcall:" + json.dumps({"name": "thread_launch", "arguments": {"machine": link, "project": "p1", "prompt": "mode:plain refused"}})})
        refused = until(lambda: next((e for e in log_of(ha) if e.get("$") == "MessagePosted" and e.get("thread") == tha and "MCPCALL thread_launch" in e.get("text", "") and "no machine of yours" in e.get("text", "")), None))
        check("thread_launch to an unverified link is refused on A", bool(refused))
        rpc(B, "bots.post", {"room": "rfar", "text": "after unverify"})
        time.sleep(3)
        check("a post from B is not taken by A", not any(e.get("$") == "RoomPosted" and e.get("text") == "after unverify" for e in log_of(ha)))
        check("nothing new was launched on B", not any(e.get("$") == "ThreadCreated" and "refused" in e.get("title", "") for e in log_of(hb)[before_b:]))
    finally:
        for p in (pa, pb):
            p.terminate()
        for p in (pa, pb):
            try:
                p.wait(10)
            except subprocess.TimeoutExpired:
                p.kill()
        if os.environ.get("KEEP") != "1":
            shutil.rmtree(root, ignore_errors=True)
        else:
            print("kept", root)
    return finish()


if __name__ == "__main__":
    sys.exit(main())
