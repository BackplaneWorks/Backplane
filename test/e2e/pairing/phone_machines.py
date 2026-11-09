#!/usr/bin/env python3
"""Your machines on a phone: data/1 against two linked temp-home hubs (unified.py's rig: A and B, one repository on both, B with a board, a STEP model and an
uncommitted change). A data/1 socket on A (the phone's hub) lists B's projects and threads with marks, filters by machine, and drives B's thread: entries,
message.send, turn.interrupt, thread.act (settle, unsettle, archive, unarchive), thread.create in B's project, link.resolve by machine name, the viewers
(viewer.files, viewer.watch, files.entries, viewer.diff) and a terminal (term.new, term.write, term.watch), and pushes when B's thread moves.

  phone_machines.py --hub PATH/backplane-hub [--port-a 3841] [--port-b 3842] [--hold DIR]
  --hold DIR: write DIR/rig.json ({url, token_file, far_thread, far_project, machine} of A) and keep both hubs up until DIR/stop exists (a phone to look at).
"""
import argparse, base64, json, os, shutil, subprocess, struct, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, Socket, check, report, finish  # noqa: E402
from link import rpc  # noqa: E402
import bots, unified  # noqa: E402


class Data(Socket):
    """A data/1 JSON socket: requests by name, the pushes it heard kept."""
    def __init__(self, hub, token):
        super().__init__(hub, token)
        self.s.settimeout(30); self.next = 0; self.pushes = []; self.bins = []
        self.hello_frame = self.hello()

    def send(self, obj):
        data, mask = json.dumps(obj).encode(), os.urandom(4); n = len(data)
        head = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n) if n < 65536 else bytes([0x80 | 127]) + struct.pack(">Q", n))
        self.s.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def pump(self, seconds, until=None):
        end = time.time() + seconds
        while time.time() < end:
            self.s.settimeout(max(0.1, end - time.time()))
            try: op, body = self.frame()
            except (OSError, EOFError): return None
            if op == 2: self.bins.append(body); continue
            if op != 1: continue
            j = json.loads(body)
            if j.get("t") == "d": self.pushes.append(j)
            if until and until(j): return j
        return None

    def call(self, m, p=None, wait=60):
        self.next += 1; i = self.next
        self.send({"id": i, "m": m, "p": p or {}})
        r = self.pump(wait, lambda j: j.get("t") == "reply" and j.get("id") == i)
        if r is None: return {"ok": False, "error": "no reply"}
        return r

    def ok(self, m, p=None, wait=60):
        r = self.call(m, p, wait)
        return r.get("result") if r.get("ok") else {"_error": r.get("error")}


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--hub", required=True); a.add_argument("--port-a", type=int, default=3841); a.add_argument("--port-b", type=int, default=3842); a.add_argument("--hold", default="")
    o = a.parse_args()
    bun = shutil.which("bun")
    if not bun:
        report("phone machines", "not run", "bun (the stand-in agent's runtime) is not installed"); return finish()
    base = os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")); os.makedirs(base, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-phonem-", dir=base)
    agents = os.path.join(root, "bin"); os.makedirs(agents)
    with open(os.path.join(agents, "claude"), "w") as f: f.write(f'#!/bin/sh\nexec {bun} {bots.ROOT}/test/tools/claude_standin.ts "$@"\n')
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(agents, n), "w") as f: f.write("#!/bin/sh\nexit 1\n")
    for n in os.listdir(agents): os.chmod(os.path.join(agents, n), 0o755)
    unified.seed(root, "a", "git@github.com:o/r.git", False); unified.seed(root, "b", "https://github.com/o/r", True)
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
        inv = rpc(A, "bots.invite"); j = rpc(B, "bots.join", {"invite": inv["invite"]}); link = inv["peer"]
        check("linked", bool(j.get("peer")), json.dumps(j)[:200]); time.sleep(1)
        for hub in (A, B):
            r = rpc(hub, "bots.peer_own", {"peer": link, "own": True}); check("link marked as the owner's own", r.get("ok") is True, json.dumps(r)[:200])
        rpc(B, "bots.create", {"name": "bob"})
        d = Data(A, A.root)
        check("hello says machines", "machines" in (d.hello_frame or {}).get("features", []))
        # (the origins are asked for once a minute; B's threads arrive by the mirror)
        def listed():
            r = d.ok("threads.list", {})
            ts = r.get("threads") or []
            return r if any(t["id"].endswith("~t-b") for t in ts) and any(t["id"].endswith("~u-b") for t in ts) else None
        tl = bots.until(listed, 150)
        if not check("threads.list holds B's threads", bool(tl), json.dumps(d.ok("threads.list", {}))[:400]): return finish()
        ts = {t["id"]: t for t in tl["threads"]}
        far = next(t for k, t in ts.items() if k.endswith("~t-b")); mine = ts.get("t-a", {})
        check("A's own thread has no mark", "t-a" in ts and not mine.get("mark"), json.dumps(mine)[:200])
        mk = far.get("mark") or {}
        check("B's thread has its machine's mark", mk.get("name") and mk.get("away") is False and mk.get("runs", "").startswith("runs on "), json.dumps(far)[:300])
        order = [t["updatedAt"] for t in tl["threads"]]
        check("threads are newest first across machines", order == sorted(order, reverse=True), str(order))
        # the repository is one project (once B's origin is known), the other is B's alone
        def projects():
            r = d.ok("projects.list", {}); ps = r.get("projects") or []
            repo = next((p for p in ps if p["title"] == "repo-a"), None)
            return r if repo and repo.get("also") and any(p["title"] == "other-b" for p in ps) else None
        pl = bots.until(projects, 150)
        if not check("projects.list: the repository once with `also`, B's other project with its mark", bool(pl), json.dumps(d.ok("projects.list", {}))[:500]): return finish()
        repo = next(p for p in pl["projects"] if p["title"] == "repo-a"); other = next(p for p in pl["projects"] if p["title"] == "other-b")
        check("no row of its own for B's copy of the repository", not any(p["title"] == "repo-b" for p in pl["projects"]))
        short = mk.get("short")
        check("also, marks and newOn on the shared project", repo["also"] == f"also on {short}" and repo["marks"][0]["name"] == mk["name"] and [x["value"] for x in repo["newOn"]][0] == "p1" and len(repo["newOn"]) == 2, json.dumps(repo)[:500])
        check("B's own project says where it is", other["mark"]["label"] == f"on {short}" and other["newOn"][0]["value"] == other["id"] and "~" in other["id"], json.dumps(other)[:300])
        check("the shared project counts both machines' threads", repo["threadCount"] == 2, json.dumps(repo)[:200])
        chips = pl.get("machines") or []
        check("switcher chips: All, this machine, B", [c["value"] for c in chips] == ["", "@here", mk["name"]] and chips[0]["on"], json.dumps(chips))
        here = d.ok("threads.list", {"machine": "@here"}); check("machine @here: this machine's threads only", [t["id"] for t in here["threads"]] == ["t-a"] and here["machines"][1]["on"], json.dumps(here)[:300])
        onb = d.ok("threads.list", {"machine": mk["name"]}); check("machine B: B's threads only", all("~" in t["id"] for t in onb["threads"]) and len(onb["threads"]) >= 2, json.dumps(onb)[:300])
        pb_ = d.ok("projects.list", {"machine": mk["name"]}); check("machine B: B's projects, the repository under its own id", {p["title"] for p in pb_["projects"]} == {"repo-b", "other-b"}, json.dumps([p["title"] for p in pb_["projects"]]))
        inrepo = d.ok("threads.list", {"project": "p1"}); check("threads of the shared project include B's", {t["id"] for t in inrepo["threads"]} == {"t-a", far["id"]}, json.dumps([t["id"] for t in inrepo["threads"]]))
        # a far thread, opened, sent to, interrupted, settled, archived
        en = d.ok("thread.entries", {"thread": far["id"]}); check("thread.entries of the far thread", "entries" in en and en["thread"]["mark"]["name"] == mk["name"], json.dumps(en)[:300])
        sub = d.ok("subscribe", {"topic": "thread", "thread": far["id"]}); check("subscribe to the far thread", "sub" in sub, json.dumps(sub))
        r = d.ok("message.send", {"thread": far["id"], "text": "hello from the phone", "clientId": "pm-1"})
        check("message.send to the far thread", r.get("entry", {}).get("text") == "hello from the phone", json.dumps(r)[:300])
        got = bots.until(lambda: any(e.get("$") == "MessagePosted" and e.get("thread") == "t-b" and e.get("text") == "hello from the phone" for e in bots.log_of(hb)), 60)
        check("it was posted on B", bool(got))
        d.pump(25, lambda j: j.get("t") == "d" and j.get("k") == "entry" and j.get("thread") == far["id"])
        en2 = d.ok("thread.entries", {"thread": far["id"]}); check("the mirror holds it too (thread.entries)", any(e.get("text") == "hello from the phone" for e in en2.get("entries", [])), json.dumps([e.get("text") for e in en2.get("entries", [])])[:300])
        check("the far thread's new entries arrive as pushes", any(p.get("k") == "entry" and p.get("thread") == far["id"] for p in d.pushes), str(len(d.pushes)))
        # the diff is the thread's checkpoint (made when its turn started) against the folder now: a change made after it
        bots.until(lambda: any(e.get("$") == "TurnChanged" and e.get("thread") == "t-b" and e.get("state") in ("completed", "idle") for e in bots.log_of(hb)[-40:]), 60)
        with open(os.path.join(root, "b", "proj", "NOTES.md"), "a") as f: f.write("a change after the turn\n")
        df = d.ok("viewer.diff", {"thread": far["id"]}); check("viewer.diff shows B's change", "a change after the turn" in df.get("diff", ""), json.dumps(df)[:300])
        r = d.ok("turn.interrupt", {"thread": far["id"]}); check("turn.interrupt on the far thread", "_error" not in r, json.dumps(r))
        for act, ev in (("settle", "ThreadSettled"), ("unsettle", "ThreadUnsettled"), ("archive", "ThreadArchived"), ("unarchive", "ThreadArchived")):
            r = d.ok("thread.act", {"thread": far["id"], "action": act}); n0 = [e for e in bots.log_of(hb) if e.get("$") == ev and e.get("id") == "t-b"]
            ok = bots.until(lambda: len([e for e in bots.log_of(hb) if e.get("$") == ev and e.get("id") == "t-b"]) >= 1, 30)
            check(f"thread.act {act} on the far thread reaches B", "_error" not in r and bool(ok), json.dumps(r)[:200])
        # a thread started in B's project from here
        r = d.ok("thread.create", {"project": other["id"], "text": "start on B", "title": "from phone"})
        check("thread.create in a far project", r.get("thread", {}).get("id", "").endswith("~") is False and "~" in r.get("thread", {}).get("id", ""), json.dumps(r)[:300])
        made = bots.until(lambda: next((e for e in bots.log_of(hb) if e.get("$") == "ThreadCreated" and e.get("project") == "p2" and e.get("title") != "other thread"), None), 30)
        check("B made it in its project", bool(made))
        r = d.ok("thread.create", {"project": "p1", "title": "via newOn"}) if False else None
        # links
        lr = d.ok("link.resolve", {"url": f"backplane://thread/{mk['name']}~t-b"}); check("backplane://thread/<machine>~<id> resolves", lr.get("kind") == "thread" and lr["thread"] == far["id"], json.dumps(lr))
        # viewers
        vf = d.ok("viewer.files", {"thread": far["id"]}); check("viewer.files of the far thread names B's board", vf.get("pcb", "").endswith("ecc83.kicad_pcb"), json.dumps(vf)[:300])
        fe = d.ok("files.entries", {"thread": far["id"], "path": ""}); check("files.entries lists B's folder", any(e["name"] == "NOTES.md" for e in fe.get("entries", [])), json.dumps(fe)[:300])
        n = len(d.bins); w = d.ok("viewer.watch", {"thread": far["id"], "kind": "board"}, 90)
        d.pump(30, lambda j: len(d.bins) > n)
        check("viewer.watch board: plot frames arrive from B", "_error" not in w and len(d.bins) > n, json.dumps(w)[:200] + f" frames={len(d.bins) - n}")
        d.ok("viewer.watch", {"thread": far["id"], "kind": ""})
        n = len(d.bins); w = d.ok("viewer.watch", {"thread": far["id"], "kind": "3d"}, 90); d.pump(40, lambda j: len(d.bins) > n)
        check("viewer.watch 3d: the model arrives from B", "_error" not in w and len(d.bins) > n, json.dumps(w)[:200] + f" frames={len(d.bins) - n}")
        d.ok("viewer.watch", {"thread": far["id"], "kind": ""})
        t = d.ok("term.new", {"thread": far["id"], "cols": 80, "rows": 24}, 60); check("term.new on the far thread", t.get("key") == far["id"], json.dumps(t)[:200])
        d.ok("term.watch", {"key": far["id"]}); d.ok("term.write", {"key": far["id"], "text": "echo far-$((6*7))\r"})
        seen = []
        def termtxt(j):
            if j.get("t") == "d" and j.get("k") == "term" and j.get("key") == far["id"]: seen.append(base64.b64decode(j.get("b", "")).decode("utf8", "replace"))
            return "far-42" in "".join(seen)
        d.pump(30, termtxt); check("terminal output of B's shell reaches the phone", "far-42" in "".join(seen), "".join(seen)[-200:])
        if o.hold:
            os.makedirs(o.hold, exist_ok=True)
            with open(os.path.join(o.hold, "rig.json"), "w") as f: json.dump({"url": f"ws://127.0.0.1:{o.port_a}", "token_file": os.path.join(ha, "token"), "far_thread": far["id"], "far_project": other["id"], "machine": mk["name"]}, f)
            print("holding: touch", os.path.join(o.hold, "stop"), flush=True)
            end = time.time() + 3 * 3600
            while time.time() < end and not os.path.exists(os.path.join(o.hold, "stop")): time.sleep(1)
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
