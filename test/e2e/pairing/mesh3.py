#!/usr/bin/env python3
"""Three real hubs, every pair linked, and everything between every pair. Temp homes, ports 3831-3833, --no-tailscale, the stand-in agent.

  mesh3.py --hub PATH/backplane-hub [--port 3831]

  - A-B and B-C are linked the way link.py links two (bots.invite / bots.join, marked Mine); A-C is NOT: A sees C in its machine list through B
    (`mesh.machines refresh`, the Machines page's "Pair" row), presses it (machines.page.act machine-introduce) and C approves once
    (machines.requests / machines.approve): a code made by C for B's asking, then the ordinary request that waits for approval;
  - machine names (BACKPLANE_NAME) are the words `~name` and `name@machine` use; a rename lands; a clash is refused;
  - a room with a bot on each of the three: one copy per hub with the same members, each post once on each hub, none sent back;
  - thread_launch and thread_send across all six directed pairs, and plane_read all six ways;
  - a `~machine` word in a message resolves to the machine, its status and its projects, and the prompt lists the paired machines.

Needs bun (the stand-in agent). Protocol result, not a client one. Exit 1 when a check failed."""
import argparse, json, os, shutil, subprocess, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, report, finish  # noqa: E402
from link import rpc  # noqa: E402
from bots import log_of, until, ROOT  # noqa: E402

NAMES = ["ma", "mb", "mc"]


def start(hub_bin, root, i, port, ports, agents):
    name = NAMES[i]
    home = os.path.join(root, name, "home"); os.makedirs(home, exist_ok=True)
    proj = os.path.join(root, name, "proj"); os.makedirs(proj, exist_ok=True)
    subprocess.run(["git", "init", "-q"], cwd=proj, check=True)
    with open(os.path.join(home, "events.jsonl"), "a") as f:
        f.write(json.dumps({"$": "ProjectCreated", "id": "p1", "title": "proj-" + name, "root": proj, "at": 1}) + "\n")
    env = {"PATH": agents + ":/usr/bin:/bin", "HOME": os.path.join(root, name), "BACKPLANE_NO_UPDATE": "1", "BACKPLANE_AGENTS": agents, "BACKPLANE_NAME": name,
           "BACKPLANE_ORDER_SERVER": "http://127.0.0.1:9", "BACKPLANE_KEYRING_LOCKED": "0",
           "BACKPLANE_PEERS": ",".join(f"http://127.0.0.1:{p}" for p in ports if p != port), "BACKPLANE_URL": f"http://127.0.0.1:{port}",
           "FAKE_LOG": os.path.join(root, name, "standin.log"), "BP_STANDIN_MODE": "stream", "BP_STREAM_N": "2", "BP_STREAM_MS": "50"}
    p = subprocess.Popen([hub_bin, "--home", home, "--port", str(port), "--no-tailscale", "--foreground"], env=env,
                         stdout=open(os.path.join(root, name, "hub.log"), "w"), stderr=subprocess.STDOUT)
    return p, home


def link(X, Y, label):
    inv = rpc(X, "bots.invite")
    j = rpc(Y, "bots.join", {"invite": inv["invite"]})
    check(f"{label} linked", bool(j.get("peer")), json.dumps(j)[:200])
    for h in (X, Y):
        r = rpc(h, "bots.peer_own", {"peer": inv["peer"], "own": True})
        check(f"{label} marked Mine", r.get("ok") is True, json.dumps(r)[:160])
    return inv["peer"]


def main():
    a = argparse.ArgumentParser(); a.add_argument("--hub", required=True); a.add_argument("--port", type=int, default=3831)
    o = a.parse_args()
    if not shutil.which("bun"):
        report("three hubs", "not run", "bun (the stand-in agent's runtime) is not installed"); return finish()
    bun = shutil.which("bun")
    base = os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")); os.makedirs(base, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-mesh3-", dir=base)
    agents = os.path.join(root, "bin"); os.makedirs(agents)
    with open(os.path.join(agents, "claude"), "w") as f: f.write(f'#!/bin/sh\nexec {bun} {ROOT}/test/tools/claude_standin.ts "$@"\n')
    os.chmod(os.path.join(agents, "claude"), 0o755)
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(agents, n), "w") as f: f.write("#!/bin/sh\nexit 1\n")
        os.chmod(os.path.join(agents, n), 0o755)
    ports = [o.port + i for i in range(3)]
    procs, homes = [], []
    for i in range(3):
        p, h = start(o.hub, root, i, ports[i], ports, agents); procs.append(p); homes.append(h)
    H = [Hub(f"http://127.0.0.1:{p}", "") for p in ports]
    try:
        if not all(h.wait_up(60) for h in H):
            check("three hubs up", False, open(os.path.join(root, "ma", "hub.log")).read()[-300:]); return finish()
        for h, home in zip(H, homes): h.root = open(os.path.join(home, "token")).read().strip()
        A, B, C = H
        lab = link(A, B, "A-B"); lbc = link(B, C, "B-C")
        time.sleep(2)

        # A learns of C through B and pairs with it in one approval (on C)
        m = rpc(A, "mesh.machines", {"refresh": True})
        pg = rpc(A, "machines.page")
        via = [r for r in pg.get("rows", []) if any(b.get("action") == "machine-introduce" for b in r.get("buttons", []))]
        check("A is offered C, the machine B is linked to", len(via) == 1, json.dumps(pg)[:300])
        if via:
            val = next(b["value"] for b in via[0]["buttons"] if b["action"] == "machine-introduce")
            r = rpc(A, "machines.page.act", {"action": "machine-introduce", "value": val, "fields": {}})
            check("A presses Pair on C", r.get("ok") is True, json.dumps(r)[:200])
            req = until(lambda: next(iter(rpc(C, "machines.requests").get("requests", [])), None), 30)
            check("C is asked to approve, once", bool(req), "no request waiting on C")
            if req:
                check("the check code is shown on both ends", bool(req.get("check")) and req["check"] == r.get("check"), f"{req.get('check')} vs {r.get('check')}")
                rpc(C, "machines.approve", {"request": req["id"]})
        linked = until(lambda: len(rpc(A, "mesh.machines")["machines"]) == 2 and len(rpc(C, "mesh.machines")["machines"]) == 2, 40)
        check("A and C are linked directly", bool(linked), json.dumps([[(r.get("label"), r.get("note")) for r in rpc(h, "machines.page").get("rows", [])] for h in (A, C)])[:600])
        for h, n in zip(H, NAMES):
            m = rpc(h, "mesh.machines", {"refresh": True})
            check(f"{n} lists the other two by name", sorted(x["name"] for x in m["machines"]) == sorted(set(NAMES) - {n}), json.dumps(m["machines"])[:200])
            check(f"{n}'s prompt lists the paired machines", all(f"~{x}" in m["prompt"] for x in set(NAMES) - {n}), m["prompt"][:200])
        ids = {rpc(h, "mesh.machines")["mid"] for h in H}
        check("three machine ids", len(ids) == 3)

        # names: a rename lands, a clash is refused
        r = rpc(A, "machines.page.act", {"action": "machine-rename", "value": "", "fields": {"rfrom": "mb", "rname": "bee"}})
        check("A renames mb to bee", r.get("ok") is True, json.dumps(r)[:200])
        r = rpc(A, "machines.page.act", {"action": "machine-rename", "value": "", "fields": {"rfrom": "bee", "rname": "mc"}})
        check("a name another machine has is refused", r.get("ok") is False, json.dumps(r)[:200])
        time.sleep(1)
        check("A calls it bee", any(x["name"] == "bee" for x in rpc(A, "mesh.machines")["machines"]))
        rpc(A, "machines.page.act", {"action": "machine-rename", "value": "", "fields": {"rfrom": "bee", "rname": "mb"}})
        time.sleep(1)

        # a ~machine word
        m = rpc(A, "mesh.machines", {"text": "do this on ~mb and ~mc, not ~nowhere"})
        check("~machine words resolve to the machine, its status and how to launch there", len(m["lines"]) == 2 and all("thread_launch" in x for x in m["lines"]) and "here" in m["lines"][0], json.dumps(m["lines"])[:300])

        # bots and threads
        bots = [rpc(h, "bots.create", {"name": n})["bot"] for h, n in zip(H, ["ada", "bob", "cy"])]
        ths = [next(e["thread"] for e in log_of(home) if e.get("$") == "BotSet" and e.get("id") == b) for home, b in zip(homes, bots)]
        until(lambda: all(any(e.get("$") == "WorktreeSet" and e.get("id") == t for e in log_of(home)) for home, t in zip(homes, ths)))

        # a room with a bot on all three
        r = rpc(A, "bots.room", {"room": "rmesh", "name": "mesh lab", "members": f"{bots[0]}\nbob@mb\ncy@mc"})
        check("A makes a room with bots on B and C", r.get("ok") is True, json.dumps(r)[:200])
        for n, home in zip(NAMES[1:], homes[1:]):
            check(f"the room is on {n}", bool(until(lambda: any(e.get("$") == "RoomSet" and e.get("id") == "rmesh" for e in log_of(home)))))
        time.sleep(2)
        for i, h in enumerate(H):
            rpc(h, "bots.post", {"room": "rmesh", "text": f"hello from {NAMES[i]}"})
        for n, home in zip(NAMES, homes):
            ok = until(lambda: sum(1 for e in log_of(home) if e.get("$") == "RoomPosted" and e.get("room") == "rmesh") >= 3)
            check(f"{n} has all three posts", bool(ok))
        time.sleep(3)
        for n, home in zip(NAMES, homes):
            k = sum(1 for e in log_of(home) if e.get("$") == "RoomPosted" and e.get("room") == "rmesh")
            sets = sum(1 for e in log_of(home) if e.get("$") == "RoomSet" and e.get("id") == "rmesh")
            check(f"{n}: each post once, the room not passed round", k == 3 and sets <= 3, f"posts {k} room sets {sets}")
        for n, home, t, others in zip(NAMES, homes, ths, [(1, 2), (0, 2), (0, 1)]):
            told = until(lambda: all(any(e.get("$") == "MessagePosted" and e.get("thread") == t and f"hello from {NAMES[j]}" in e.get("text", "") for e in log_of(home)) for j in others))
            check(f"the bot on {n} heard both others", bool(told))

        # every directed pair: launch, send, plane_read
        planes = {}
        for i in range(3):
            for j in range(3):
                if i == j: continue
                X, Y, hx, hy, tx = H[i], H[j], homes[i], homes[j], ths[i]
                lk = next(x["link"] for x in rpc(X, "mesh.machines")["machines"] if x["name"] == NAMES[j])
                call = {"name": "thread_launch", "arguments": {"machine": NAMES[j], "project": "p1", "prompt": f"mode:plain {NAMES[i]} to {NAMES[j]}", "title": f"from {NAMES[i]}"}}
                rpc(X, "bots.say", {"bot": bots[i], "text": "mcpcall:" + json.dumps(call)})
                launched = until(lambda: next((e for e in log_of(hy) if e.get("$") == "ThreadLaunched" and e.get("by") == f"{lk}~{tx}"), None), 40)
                check(f"{NAMES[i]} launches a thread on {NAMES[j]} by its name", bool(launched))
                if launched:
                    txt = f"send {NAMES[i]}>{NAMES[j]}"
                    rpc(X, "bots.say", {"bot": bots[i], "text": "mcpcall:" + json.dumps({"name": "thread_send", "arguments": {"threadId": f"{lk}~{launched['id']}", "text": txt}})})
                    got = until(lambda: any(e.get("$") == "MessagePosted" and e.get("thread") == launched["id"] and e.get("text") == txt for e in log_of(hy)))
                    check(f"thread_send {NAMES[i]} -> {NAMES[j]}", bool(got))
                planes[i] = planes.get(i, 0) + 1
                rpc(X, "bots.say", {"bot": bots[i], "text": "mcpcall:" + json.dumps({"name": "plane_read", "arguments": {"machine": NAMES[j]}})})
                plane = until(lambda: sum(1 for e in log_of(hx) if e.get("$") == "MessagePosted" and e.get("thread") == tx and "MCPCALL plane_read" in e.get("text", "") and "Status plane" in e.get("text", "")) >= planes[i], 30)
                check(f"plane_read {NAMES[i]} -> {NAMES[j]}", bool(plane))

        # C moves: restarted on another port, it names its new address (BACKPLANE_URL, as a hub behind `tailscale serve` names its https one); A and B were
        # paired with the old one and are told, signed, by C: no re-pairing
        old = ports[2]; new = o.port + 10
        procs[2].terminate(); procs[2].wait(15)
        procs[2], _ = start(o.hub, root, 2, new, ports[:2] + [new], agents)
        Cn = Hub(f"http://127.0.0.1:{new}", ""); Cn.wait_up(60)
        def moved():
            m = rpc(A, "mesh.machines")["machines"]
            return any(x["name"] == "mc" and not x["away"] for x in m)
        check("A reaches C at its new address without pairing again", bool(until(moved, 90)))
        urls = lambda home: [e["url"] for e in log_of(home) if e.get("$") == "PeerSet" and e.get("name", "x") != "" and e.get("url")]
        kept = lambda h: any(str(new) in e.get("url", "") for e in log_of(h) if e.get("$") == "PeerSet" and e.get("url"))
        check("A and B keep C's new address (told by C, signed)", bool(until(lambda: kept(homes[0]) and kept(homes[1]), 60)))
        # (A lists C again once its finder has looked at the new address: asked again at once, so within seconds)
        got = None
        for attempt in range(1, 5):
            rpc(A, "bots.say", {"bot": bots[0], "text": "mcpcall:" + json.dumps({"name": "plane_read", "arguments": {"machine": "mc"}})})
            got = until(lambda: sum(1 for e in log_of(homes[0]) if e.get("$") == "MessagePosted" and e.get("thread") == ths[0] and "MCPCALL plane_read" in e.get("text", "") and "Status plane" in e.get("text", "")) >= planes[0] + 1, 20)
            if got: break
        check("plane_read A -> C works at the new address", bool(got), f"after {attempt} tries")
    finally:
        for p in procs: p.terminate()
        for p in procs:
            try: p.wait(10)
            except subprocess.TimeoutExpired: p.kill()
        if os.environ.get("KEEP") != "1": shutil.rmtree(root, ignore_errors=True)
        else: print("kept", root)
    return finish()


if __name__ == "__main__":
    sys.exit(main())
