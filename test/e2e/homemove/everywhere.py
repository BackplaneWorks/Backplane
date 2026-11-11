#!/usr/bin/env python3
"""Bub everywhere from its home (docs/ALWAYS-ON.md section 4), rehearsed on two temp-home hubs and a stand-in agent. Nothing of the person's is touched.

  everywhere.py HUB_BIN WORK

A plays the desktop (a project with a thread), B the always-on boat-hub (no project), on 127.0.0.1 ports 3901/3902 with --no-tailscale. They
are linked by an approved pairing (machines.code, machines.pair, machines.approve). The agent is test/tools/claude_standin.ts behind a `claude`
that answers `auth status` signed in only where its HOME has a file `signed` (the readiness check reads that, and nothing else).
Checks, in order: the readiness check refuses a move to a machine that is not signed in, says why in Bub's row, and passes once it is; a message
sent to Bub mid-move is held and reaches the new home; Bub's row says "Moving to hub-b" during the copy; after the move a message to the old copy
goes on to B; from B (no project) Bub lists machines, lists, launches, sends, reads, renames and moves the provider of a thread on A, and a tool
that needs a local thing says so; with A away a launch and a send are held and go when A is back; the ring's one address (B's /hook/<id>) lands
where the ring is owned before, after and back; the move back. Prints one line per check and a summary table. Exit 1 when a check failed.
Needs bun. Python 3.9, standard library only.
"""
import json, os, shutil, sys, time
sys.argv = sys.argv[:3]
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import move as m  # noqa: E402
from proto import check, finish  # noqa: E402

m.PORTS.update({"A": 3901, "B": 3902})
os.environ["BACKPLANE_MOVE_PAUSE_MS"] = "4000"  # the copy is held open so a message is sent mid-move (net/homemove.rs)
WORK = m.WORK
ROOT = os.path.abspath(os.path.join(HERE, "../../.."))
TABLE = []


def item(name, before, ok, detail=""):
    TABLE.append((name, before, "yes" if ok else "NO"))
    return check(name, ok, detail)


def agents():
    d = os.path.join(WORK, "agents")
    os.makedirs(d, exist_ok=True)
    bun = shutil.which("bun") or next((b for b in [os.path.expanduser("~/.bun/bin/bun")] if os.path.exists(b)), None)
    assert bun, "needs bun (the stand-in agent runs on it)"
    with open(os.path.join(d, "claude"), "w") as f:
        f.write("#!/bin/sh\n"
                "if [ \"$1\" = auth ] && [ \"$2\" = status ]; then\n"
                "  if [ -f \"$HOME/signed\" ]; then echo '{\"loggedIn\":true,\"email\":\"standin@example.com\"}'; exit 0; fi\n"
                "  echo '{\"loggedIn\":false}'; exit 1\nfi\n"
                "export FAKE_LOG=\"$HOME/standin.log\" BP_STANDIN_MODE=stream BP_STREAM_N=1 BP_STREAM_MS=20; printf '%s\\n' \"$*\" | cut -c1-2000 >> \"$HOME/argv.log\"\n"
                f"exec {bun} {ROOT}/test/tools/claude_standin.ts \"$@\"\n")
    os.chmod(os.path.join(d, "claude"), 0o755)


def seed_desktop():
    home = os.path.join(WORK, "A")
    os.makedirs(home, exist_ok=True)
    proj = os.path.join(WORK, "proj")
    os.makedirs(proj, exist_ok=True)
    if not os.path.exists(os.path.join(proj, ".git")):
        os.system(f"git -C {proj} init -q && git -C {proj} -c user.email=r@x -c user.name=rig commit -q --allow-empty -m init")
    ev = os.path.join(home, "events.jsonl")
    if not os.path.exists(ev):
        with open(ev, "w") as f:
            f.write(json.dumps({"$": "ProjectCreated", "id": "p1", "title": "board", "root": proj, "at": 1}) + "\n")
            f.write(json.dumps({"$": "ThreadCreated", "id": "t1700000001-1", "project": "p1", "title": "Desk work", "env": "local", "branch": "", "worktree": "",
                                "provider": "claude", "model": "", "at": 2}) + "\n")


def thread_of(hub, bot):
    return next((e["thread"] for e in m.events(hub) if e.get("$") == "BotSet" and e.get("id") == bot and e.get("thread")), "")


def said(hub, thread, text):
    return any(e.get("$") == "MessagePosted" and e.get("thread") == thread and text in e.get("text", "") for e in m.events(hub))


def reply_of(hub, thread, tool, since):
    """The stand-in's answer to an mcpcall (its reply, `MCPCALL <tool> <json-rpc answer>`, cut at 600 characters): (ok, the answer's text)."""
    for e in m.events(hub)[since:]:
        t = e.get("text", "")
        if e.get("$") == "MessagePosted" and e.get("thread") == thread and e.get("role") == "assistant" and t.startswith(f"MCPCALL {tool} "):
            body = t[len(f"MCPCALL {tool} "):]
            return ('"isError":true' not in body, body.replace('\\"', '"'))
    return None


def bub_call(hub, bub, thread, tool, args, wait=60):
    n = len(m.events(hub))
    m.rpc(hub, "bots.say", {"bot": bub, "text": "mcpcall:" + json.dumps({"name": tool, "arguments": args})})
    return m.wait(lambda: reply_of(hub, thread, tool, n), wait) or (False, "no answer")


def row_note(hub, uber_only=True):
    """Bub's sidebar row as this hub draws it (the note), from the setting the row reads: home.moving."""
    return m.setting(hub, "home.moving")


def main():
    agents()
    seed_desktop()
    A, B = m.start("A"), m.start("B")
    code = m.rpc(B, "machines.code")
    m.rpc(A, "machines.pair", {"address": B.base, "code": code.get("code", "")})
    req = m.wait(lambda: (m.rpc(B, "machines.requests") or {}).get("requests"))
    m.rpc(B, "machines.approve", {"request": req[0]["id"]})
    check("A and B linked", bool(m.wait(lambda: any(e.get("$") == "PeerSet" and not e.get("revoked") for e in m.events(A)))))
    open(os.path.join(WORK, "home-A", "signed"), "w").close()
    bub = m.rpc(A, "bots.create", {"name": "Bub"}).get("bot")
    m.rpc(A, "bots.uber", {"bot": bub})
    m.wait(lambda: m.uber(A) == bub, 20)
    tb = m.wait(lambda: thread_of(A, bub), 20)
    hook = m.rpc(A, "bots.hook", {"bot": bub, "name": "Pebble Index"})
    hid = hook.get("hook", "")
    tok = m.rpc(A, "bots.hook_token", {"hook": hid}).get("token", "")
    fr = m.rpc(A, "ring.front.use", {"to": "hub-b"})
    item("ring: B's /hook/<id> lands on A while A owns it", "yes", m.post_hook(B.base, hid, tok) == 200, json.dumps(fr)[:160])

    # 5. the readiness check
    c = m.rpc(A, "home.check", {"to": "hub-b"})
    item("check: refuses a target with no Claude sign-in", "no (moved, Bub could not answer)", c.get("ready") is False and any("not signed in on hub-b" in r for r in c.get("reasons", [])), json.dumps(c)[:300])
    r = m.rpc(A, "home.move", {"to": "hub-b"})
    item("move button refuses with the reason", "no", r.get("ok") is False and "not signed in" in (r.get("error") or ""), json.dumps(r)[:200])
    item("Bub's row says the move did not happen, once", "no", '"failed"' in row_note(A) and m.uber(A) == bub)
    open(os.path.join(WORK, "home-B", "signed"), "w").close()
    c = m.rpc(A, "home.check", {"to": "hub-b"})
    item("check: passes once signed in (build, link, disk, memory)", "n/a", c.get("ready") is True, json.dumps(c)[:300])

    # 2. the move, with a message mid-move
    r = m.rpc(A, "home.move", {"to": "hub-b"})
    check("home.move starts", r.get("started") is True, json.dumps(r)[:200])
    item("Bub's row says Moving to hub-b during the copy", "no", '"moving"' in row_note(A) and "hub-b" in row_note(A), row_note(A))
    mid = m.rpc(A, "turn.start", {"thread": tb, "text": "zz mid-move hello", "msg": "mmid1"})
    item("a message sent mid-move is answered ok (held)", "dropped into the old copy", mid.get("ok", True) is not False, json.dumps(mid)[:200])
    done = m.wait(lambda: (lambda s: s if s.get("state") in ("done", "failed") else None)(m.rpc(A, "home.status")), 180)
    check("the move is done", (done or {}).get("state") == "done", json.dumps(done))
    check("B's uberbot is Bub (same id)", m.uber(B) == bub)
    item("the mid-move message reaches Bub on B", "no", bool(m.wait(lambda: said(B, tb, "zz mid-move hello"), 60)))
    item("Bub's row: no move note after the move", "n/a", row_note(A) == "", row_note(A))
    m.rpc(A, "turn.start", {"thread": tb, "text": "zz to the old copy", "msg": "mold1"})
    item("a message to the old copy of Bub's thread goes on to B", "no (ran on A's copy)", bool(m.wait(lambda: said(B, tb, "zz to the old copy"), 60)))
    item("ring: B's /hook/<id> lands on B after the move", "yes", m.post_hook(B.base, hid, tok) == 200)

    # 1. from B (no project), Bub works on A
    link_a = next(e["id"] for e in m.events(B) if e.get("$") == "PeerSet" and not e.get("revoked"))
    tA = f"{link_a}~t1700000001-1"
    ok, v = bub_call(B, bub, tb, "machine_list", {})
    item("machine_list from B shows A's project", "yes", ok and '"id":"p1"' in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_list", {"machine": "hub-a"})
    item("thread_list {machine} lists A's threads", "yes", ok and tA in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_launch", {"machine": "hub-a", "project": "p1", "prompt": "zz launched from B", "title": "From B"})
    item("thread_launch {machine, project} into A's project", "yes", ok and '"state":"started"' in v and bool(m.wait(lambda: any(e.get("$") == "MessagePosted" and "zz launched from B" in e.get("text", "") for e in m.events(A)), 30)), str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_send", {"threadId": tA, "text": "zz send from B"})
    item("thread_send on <link>~<id>", "yes", ok and bool(m.wait(lambda: said(A, "t1700000001-1", "zz send from B"), 30)), str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_read", {"threadId": tA, "limit": 3})
    item("thread_read on <link>~<id>", "yes", ok and f'"id":"{tA}"' in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_wait", {"threadId": tA, "timeoutMs": 20000}, 60)
    item("thread_wait on <link>~<id>", "yes", ok and '"done"' in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_update", {"threadId": tA, "title": "Renamed from B"})
    item("thread_update title on <link>~<id>", "yes", ok and "Renamed from B" in v and any(e.get("$") == "ThreadRenamed" and e.get("title") == "Renamed from B" for e in m.events(A)), str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_update", {"threadId": tA, "model": "claude-haiku-5-5"})
    item("thread_update model on <link>~<id>", "no (not in the Rust hub)", ok and '"model":"claude-haiku-5-5"' in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_interrupt", {"threadId": tA})
    item("thread_interrupt on <link>~<id>", "yes", ok, str(v)[:200])
    board = os.path.join(WORK, "proj", "main.kicad_pcb")
    ok, v = bub_call(B, bub, tb, "kicad_ipc", {"path": board, "type": "kiapi.board.commands.GetItems"})
    item("kicad_ipc on A's board from B says where to do it", "no (a KiCad error)", not ok and "is not on this machine" in v and "thread_launch" in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "viewer_show", {"path": board})
    item("viewer_show of A's board from B says where to do it", "no (showed nothing)", not ok and "is not on this machine" in v, str(v)[:200])

    # 1. the desktop away: held launch and send
    m.stop("A")
    ok, v = bub_call(B, bub, tb, "thread_launch", {"machine": "hub-a", "project": "p1", "prompt": "zz held launch", "title": "Held"})
    item("thread_launch with A away is held (queued)", "yes", ok and '"state":"queued"' in v, str(v)[:200])
    ok, v = bub_call(B, bub, tb, "thread_send", {"threadId": tA, "text": "zz held send"})
    item("thread_send with A away is held", "yes", ok and '"state":"held"' in v, str(v)[:200])
    item("ring: B's /hook/<id> lands on B with A stopped", "yes", m.post_hook(B.base, hid, tok) == 200)
    A = m.start("A")
    item("the held launch starts on A when it is back", "yes", bool(m.wait(lambda: any(e.get("$") == "MessagePosted" and "zz held launch" in e.get("text", "") for e in m.events(A)), 60)))
    item("the held send reaches A when it is back", "yes", bool(m.wait(lambda: said(A, "t1700000001-1", "zz held send"), 60)))

    # and back
    r = m.rpc(A, "home.move", {"to": "back"})
    check("home.move back starts", r.get("started") is True, json.dumps(r)[:200])
    item("A's row says Moving to hub-a during the move back", "no", '"moving"' in row_note(A), row_note(A))
    back = m.wait(lambda: m.uber(A) == bub and m.uber(B) == "", 180)
    item("Bub is back on A, same id", "yes", bool(back))
    item("A's row: no move note after the move back", "n/a", bool(m.wait(lambda: row_note(A) == "", 30)), row_note(A))
    item("ring: B's /hook/<id> lands on A again", "yes", bool(m.wait(lambda: m.post_hook(B.base, hid, tok) == 200, 30)))
    m.stop("A"); m.stop("B")
    print("\n| item | worked before | works now |\n|---|---|---|")
    for n, b, w in TABLE:
        print(f"| {n} | {b} | {w} |")
    finish()


if __name__ == "__main__":
    try:
        main()
    finally:
        for n in list(m.procs):
            m.stop(n)
