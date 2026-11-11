#!/usr/bin/env python3
"""Another machine's bots and rooms in this hub's clients: two temp-home hubs (bots.py's rig: stand-in agents, --no-tailscale), linked by a pairing
the person approves (machines.code on A, machines.pair on B, machines.approve on A: link.py). B has an uberbot (bub, with a memory) and a room; A has
no uberbot. Then on A:
  - the GPUI sidebar (view.open) shows B's uberbot first, as the uberbot, with B's marker, then B's other bot and B's room;
  - data/1 `bots.list` gives the same: bub with uber and mark, the room with its mark;
  - B's uberbot opens as a bot (its chat is its thread on B), its Memory tab shows B's memory, read-only;
  - a message from A's view socket (what GPUI's composer sends: turn.start) and one from data/1 (message.send) reach bub's thread on B, B's stand-in
    answers, and the answer is in A's mirror (data/1 thread.entries);
  - a post from A (bots.post, the GPUI room page; room.post, data/1) lands in B's room, and A's room.posts shows it back from the mirror.

  far_bots.py --hub PATH/backplane-hub [--port-a 3851] [--port-b 3852]

Needs bun (the stand-in is test/tools/claude_standin.ts). Exit 1 when a check failed.
"""
import argparse, json, os, shutil, subprocess, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, report, finish  # noqa: E402
from settings_revoke import View  # noqa: E402
from link import rpc, peers, wait  # noqa: E402
from phone_machines import Data  # noqa: E402
import bots  # noqa: E402


def view(hub, extra):
    v = View(hub)
    try:
        r = v.request("view.open", {"now": int(time.time() * 1000), **extra})
    finally:
        v.close()
    res = r.get("result") or r
    return (res.get("shown") if isinstance(res, dict) else None) or {}


def seed(root, name):
    home = os.path.join(root, name, "home"); os.makedirs(home, exist_ok=True)
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": int(time.time()) - 90} for k, v in (("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off"))]
    with open(os.path.join(home, "events.jsonl"), "w") as f:
        f.write(json.dumps({"$": "ProjectCreated", "id": "p1", "title": "far-" + name, "root": os.path.join(root, name, "proj"), "at": 1}) + "\n")
        for e in ev: f.write(json.dumps(e) + "\n")


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--hub", required=True); a.add_argument("--port-a", type=int, default=3851); a.add_argument("--port-b", type=int, default=3852)
    o = a.parse_args()
    bun = shutil.which("bun")
    if not bun:
        report("far bots", "not run", "bun (the stand-in agent's runtime) is not installed"); return finish()
    base = os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")); os.makedirs(base, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-farbots-", dir=base)
    agents = os.path.join(root, "bin"); os.makedirs(agents)
    with open(os.path.join(agents, "claude"), "w") as f: f.write(f'#!/bin/sh\nexec {bun} {bots.ROOT}/test/tools/claude_standin.ts "$@"\n')
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(agents, n), "w") as f: f.write("#!/bin/sh\nexit 1\n")
    for n in os.listdir(agents): os.chmod(os.path.join(agents, n), 0o755)
    seed(root, "a"); seed(root, "b")
    pa, ha = bots.start(o.hub, root, "a", o.port_a, o.port_b, agents)
    A, B = (Hub(f"http://127.0.0.1:{p}", "") for p in (o.port_a, o.port_b))
    A.wait_up(60)
    pb, hb = bots.start(o.hub, root, "b", o.port_b, o.port_a, agents)
    try:
        if not (A.wait_up(60) and B.wait_up(60)):
            check("both hubs up", False); return finish()
        pa.terminate(); pa.wait(15)
        pa, ha = bots.start(o.hub, root, "a", o.port_a, o.port_b, agents)
        if not A.wait_up(60):
            check("A back up", False); return finish()
        time.sleep(3)
        A.root = open(os.path.join(ha, "token")).read().strip(); B.root = open(os.path.join(hb, "token")).read().strip()

        # the pairing the person approves
        code = rpc(A, "machines.code")
        p = rpc(B, "machines.pair", {"address": A.base, "code": code.get("code", "")})
        req = wait(lambda: (rpc(A, "machines.requests") or {}).get("requests"))
        check("B's request waits on A with its check code", bool(req) and req[0].get("check") == p.get("check"), json.dumps(req)[:300])
        if not req: return finish()
        rpc(A, "machines.approve", {"request": req[0]["id"]})
        st = wait(lambda: (lambda r: r if isinstance(r, dict) and r.get("state") in ("linked", "refused") else None)(rpc(B, "machines.attempt", {"attempt": p["attempt"]})))
        if not check("linked by the approved pairing", bool(st) and st.get("state") == "linked", json.dumps(st)[:300]): return finish()
        link = next(iter(peers(ha)))

        # B: an uberbot with a memory, another bot, a room
        bub = rpc(B, "bots.create", {"name": "bub"})["bot"]
        rpc(B, "bots.create", {"name": "calendar"})
        u = rpc(B, "bots.uber", {"bot": bub}); check("bub is B's uberbot", u.get("ok") is True, json.dumps(u)[:200])
        rpc(B, "bots.remember", {"bot": bub, "key": "tea", "kind": "fact", "text": "likes green tea"})
        r = rpc(B, "bots.room", {"room": "rring", "name": "ring", "members": bub}); check("B makes a room", r.get("ok") is True, json.dumps(r)[:200])
        rpc(B, "bots.post", {"room": "rring", "text": "hello from B"})
        thb = next(e["thread"] for e in bots.log_of(hb) if e.get("$") == "BotSet" and e.get("id") == bub and e.get("thread"))
        fbub, fth, froom = f"{link}~{bub}", f"{link}~{thb}", f"{link}~rring"

        # A's GPUI sidebar: B's uberbot in Bub's place, its bots and its room, each marked
        def side():
            s = view(A, {})
            ids = [b.get("key") for b in s.get("bots") or []]
            return s if fbub in ids and any(r["id"].endswith(froom) for r in s.get("rooms") or []) and (s["bots"][0].get("uber") is True) else None
        s = bots.until(side, 150)
        if not check("A's sidebar shows B's uberbot first, as the uberbot, and B's room", bool(s), json.dumps({k: view(A, {}).get(k) for k in ("bots", "rooms")})[:600]): return finish()
        top = s["bots"][0]
        check("the uberbot row is B's bub with B's marker and its thread there", top["key"] == fbub and (top.get("mark") or {}).get("name") and top["nav"].endswith(fth), json.dumps(top)[:300])
        check("B's other bot is listed, marked", any(b["name"] == "calendar" and b.get("mark") for b in s["bots"]), json.dumps(s["bots"])[:400])
        room = next(r for r in s["rooms"] if r["id"].endswith(froom))
        check("B's room is listed with B's marker", bool(room.get("mark")), json.dumps(room)[:200])

        # data/1 bots.list
        d = Data(A, A.root)
        bl = d.ok("bots.list")
        b0 = next((b for b in bl.get("bots", []) if b["id"] == fbub), {})
        check("bots.list: B's bub is the uberbot, with its mark and thread", b0.get("uber") is True and b0.get("thread") == fth and (b0.get("mark") or {}).get("name"), json.dumps(bl)[:500])
        check("bots.list: B's room with its mark", any(r["id"] == froom and r.get("mark") for r in bl.get("rooms", [])), json.dumps(bl.get("rooms"))[:300])

        # B's uberbot opens as a bot: its chat is its thread on B; Memory shows B's memory, read-only
        bp = view(A, {"target": {"kind": "bot", "id": fbub}}).get("bot") or {}
        check("the far uberbot opens on its chat there", bp.get("chat") == fth and bp.get("uber") is True and bp.get("edit") is False, json.dumps(bp)[:300])
        bm = view(A, {"target": {"kind": "bot", "id": fbub}, "action": "bot-tab", "value": "memory"}).get("bot") or {}
        check("its Memory tab shows B's memory", any(m.get("text") == "likes green tea" for m in bm.get("memory") or []), json.dumps(bm)[:300])

        # chat: GPUI's composer (turn.start on the view socket) and data/1 message.send, to bub's thread on B
        r = rpc(A, "turn.start", {"thread": fth, "text": "hello bub from the desktop", "msg": "m-far-1"})
        check("turn.start to the far uberbot's thread is taken", isinstance(r, dict) and r.get("ok") is True, json.dumps(r)[:200])
        got = bots.until(lambda: any(e.get("$") == "MessagePosted" and e.get("thread") == thb and e.get("text") == "hello bub from the desktop" for e in bots.log_of(hb)), 60)
        check("it reaches bub on B", bool(got))
        def answered():
            lg = bots.log_of(hb); i = next((k for k, e in enumerate(lg) if e.get("$") == "MessagePosted" and e.get("text") == "hello bub from the desktop"), None)
            return None if i is None else next((e for e in lg[i + 1:] if e.get("$") == "MessagePosted" and e.get("thread") == thb and e.get("role") == "assistant" and e.get("text")), None)
        ans = bots.until(answered, 90)
        check("B's stand-in answers in bub's thread", bool(ans))
        if ans:
            back = bots.until(lambda: any(e.get("text") == ans["text"] for e in (d.ok("thread.entries", {"thread": fth}).get("entries") or [])), 60)
            check("the answer is back on A (the mirror of bub's thread)", bool(back), json.dumps(d.ok("thread.entries", {"thread": fth}))[:300])
        r = d.ok("message.send", {"thread": fth, "text": "hello bub from the phone", "clientId": "far-bots-1"})
        check("data/1 message.send to the far uberbot's thread", "_error" not in r, json.dumps(r)[:200])
        got = bots.until(lambda: any(e.get("$") == "MessagePosted" and e.get("thread") == thb and e.get("text") == "hello bub from the phone" for e in bots.log_of(hb)), 60)
        check("the phone's message reaches bub on B", bool(got))

        # rooms: a post from A's GPUI room page (bots.post) and from data/1 (room.post) lands in B's room
        r = rpc(A, "bots.post", {"room": froom, "text": "hello room from the desktop"})
        check("bots.post to B's room is taken", isinstance(r, dict) and r.get("ok") is True, json.dumps(r)[:200])
        r = d.ok("room.post", {"room": froom, "text": "hello room from the phone", "clientId": "far-room-1"})
        check("data/1 room.post to B's room is taken", "_error" not in r, json.dumps(r)[:200])
        for t in ("hello room from the desktop", "hello room from the phone"):
            got = bots.until(lambda: any(e.get("$") == "RoomPosted" and e.get("room") == "rring" and e.get("text") == t for e in bots.log_of(hb)), 60)
            check(f"'{t}' lands in B's room", bool(got))
        posts = bots.until(lambda: (lambda ps: ps if any(x.get("text") == "hello room from the phone" for x in ps) else None)(d.ok("room.posts", {"room": froom}).get("posts") or []), 60)
        check("A's room.posts reads B's room back from the mirror", bool(posts) and any(x.get("text") == "hello from B" for x in posts), json.dumps(d.ok("room.posts", {"room": froom}))[:300])
        rp = view(A, {"target": {"kind": "room", "id": froom}}).get("bot") or {}
        check("the GPUI room page shows B's room with its posts and marker", rp.get("kind") == "room" and rp.get("mark") and any(x.get("text") == "hello from B" for x in rp.get("posts") or []), json.dumps(rp)[:300])
    finally:
        for p in (pa, pb): p.terminate()
        for p in (pa, pb):
            try: p.wait(10)
            except subprocess.TimeoutExpired: p.kill()
        if os.environ.get("KEEP") != "1": shutil.rmtree(root, ignore_errors=True)
        else: print("kept", root)
    return finish()


if __name__ == "__main__":
    sys.exit(main())
