#!/usr/bin/env python3
"""The headless KiCad pool on a real hub with real headless KiCads (docs/kicad-ipc.md "The pool's rules"): four copies of a real board,
each project's requests through the MCP tool `kicad_ipc` of a held stand-in turn.
  pool_e2e.py WORK HUB_BIN RUNTIME BOARD_DIR [PORT]   -> a line per check, "pool e2e: N of M"; exit 1 on a failure
BOARD_DIR holds one .kicad_pro and its .kicad_pcb (a real board is only read: it is staged with test/kicad_embed/lib/real_use.sh's
stage_board and each project is a copy of that). A project "at work" is one whose agent sends requests back to back (reading the
board's tracks): a headless KiCad has no request that runs for minutes, each is bounded at 30 s by the transport.
The hub's test knob BACKPLANE_KICAD_POOL_TEST sets the pool's room and its times, so a run takes minutes: room=2 for the first hub
(whatever the machine's memory), the machine's own room for the second.
Processes are found as the hub's own children by their project path, and the only one ever killed is one captured that way."""
import glob, json, os, shutil, signal, subprocess, sys, threading, time
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
from mcp import Mcp
IN_USE, ROOM_WAIT, SPARE_IDLE = 5, 20, 15
KNOBS = f"in_use={IN_USE},room_wait={ROOM_WAIT},spare_idle={SPARE_IDLE}"
checks = []
def ok(what, cond, got=""):
    checks.append(bool(cond)); print(("ok   " if cond else "FAIL ") + what + ("" if cond and got == "" else f": {json.dumps(got, default=str)[:900]}"), flush=True)

class Hub:
    def __init__(self, work, hub_bin, rt, port, knobs):
        self.work = work; os.makedirs(work, exist_ok=True)
        env = {**os.environ, "KIPC_BENCH_HUB_ENV": json.dumps({"BACKPLANE_KICAD_POOL_TEST": knobs, "KIPC_BENCH_STANDIN_ONLY": "1"})}
        self.proc = subprocess.Popen(["bun", HERE + "/hub.mjs", hub_bin, rt, str(port), work], stdout=open(work + "/hub.out", "w"), stderr=subprocess.STDOUT, env=env, start_new_session=True)
        for _ in range(120):
            if os.path.exists(work + "/hub.json") and os.path.getsize(work + "/hub.json"): break
            if self.proc.poll() is not None: raise RuntimeError(f"hub launcher exited {self.proc.returncode}: {work}/hub.out")
            time.sleep(0.5)
        self.pid = json.load(open(work + "/hub.json"))["pid"]
    def stop(self):
        open(self.work + "/stop", "w").write("x")
        try: self.proc.wait(timeout=30)
        except Exception: pass
    def servers(self, project=None):
        """The hub's headless KiCads now: [(pid, project folder name)], its own children only."""
        out = []
        for p in glob.glob("/proc/[0-9]*"):
            try:
                cmd = open(p + "/cmdline").read().split("\0"); stat = open(p + "/stat").read().rsplit(")", 1)[1].split()
            except OSError: continue
            if "api-server" not in cmd or stat[0] == "Z": continue
            pro = next((c for c in cmd if c.endswith(".kicad_pro")), ""); pid = int(os.path.basename(p))
            if not pro.startswith(self.work + "/"): continue
            if not self.child(pid): continue
            name = os.path.basename(os.path.dirname(pro))
            if project is None or name == project: out.append((pid, name))
        return sorted(out, key=lambda x: (x[1], x[0]))
    def child(self, pid):
        for _ in range(6):
            try: pid = int(open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[1])
            except OSError: return False
            if pid == self.pid: return True
            if pid <= 1: return False
        return False
    def socket_of(self, pid):
        cmd = open(f"/proc/{pid}/cmdline").read().split("\0"); d = os.path.dirname(cmd[cmd.index("--socket") + 1])
        # The filesystem can reuse an unlinked socket's inode immediately. Include its creation/change stamp,
        # as the production transport does, so replacement at the same path remains observable.
        out = []
        for s in glob.glob(d + "/*.sock"):
            st = os.stat(s); out.append((s, st.st_dev, st.st_ino, st.st_ctime_ns))
        return sorted(out)

class Project:
    """One project and the held turn whose MCP url its requests go through."""
    def __init__(self, hub, staged, name):
        self.hub, self.name = hub, name; self.dir = f"{hub.work}/runs/{name}"; shutil.rmtree(self.dir, ignore_errors=True); shutil.copytree(staged, self.dir, symlinks=True)
        self.pcb = glob.glob(self.dir + "/*.kicad_pcb")[0]; self.hold = f"{hub.work}/hold-{name}"
        for f in (self.hold, self.hold + ".stop"):
            if os.path.exists(f): os.remove(f)
        turn = subprocess.run(["bun", HERE + "/turn.mjs", hub.work, self.dir, f"HOLD {self.hold}", "nowait"], capture_output=True, text=True,
                              env={**os.environ, "KIPC_BENCH_PROVIDER": "claude"}, timeout=90)
        if turn.returncode: raise RuntimeError(f"held turn failed for {name}: {turn.stdout}\n{turn.stderr}")
        for _ in range(100):
            if os.path.exists(self.hold) and os.path.getsize(self.hold): break
            time.sleep(0.2)
        if not os.path.exists(self.hold): raise RuntimeError(f"held turn for {name} did not start: {turn.stdout}; see {hub.work}/hub.log")
        self.url = open(self.hold).read().strip(); self.doc = None
    def call(self, ty, msg=None):
        """(answer or None, error text, seconds)"""
        t0 = time.time()
        try: return Mcp(self.url).ipc(self.pcb, ty, msg), "", time.time() - t0
        except Exception as e: return None, str(e), time.time() - t0
    def version(self): return self.call("GetVersion")
    def read(self):
        """One request of some weight: the board's tracks (the document opened first, as an agent's first call does)."""
        if self.doc is None:
            r, e, _ = self.call("GetOpenDocuments", {"type": "DOCTYPE_PCB"})
            if e: return None, e, 0
            self.doc = (r.get("documents") or [None])[0]
            if self.doc is None:
                r, e, _ = self.call("OpenDocument", {"type": "DOCTYPE_PCB", "path": self.pcb})
                if e: return None, e, 0
                self.doc = r["document"]
        r, e, s = self.call("kiapi.common.commands.GetItems", {"header": {"document": self.doc}, "types": ["KOT_PCB_TRACE"]})
        if e and ("not open" in e or "no handler" in e): self.doc = None  # a new server has no document open yet: the next read opens it
        return r, e, s
    def work(self, seconds, heartbeat=False):
        """Requests back to back for `seconds`, in a thread: .errors and .n when joined."""
        self.errors, self.n, self.stop_at = [], 0, time.time() + seconds
        def loop():
            while time.time() < self.stop_at:
                r, e, _ = self.version() if heartbeat else self.read(); self.n += 1
                if e: self.errors.append(e)
                if heartbeat: time.sleep(0.2)
        self.thread = threading.Thread(target=loop); self.thread.start(); return self
    def end(self): self.stop_at = 0; self.thread.join()
    def release(self): open(self.hold + ".stop", "w").write("x")

def later(fn):
    box = {}
    def run(): box["r"] = fn()
    t = threading.Thread(target=run); t.start(); box["t"] = t; return box
def got(box, timeout=200): box["t"].join(timeout); return box.get("r", (None, "no answer in time", timeout))

def first_hub(work, hub_bin, rt, staged, port):
    h = Hub(work, hub_bin, rt, port, "room=2," + KNOBS)
    try:
        a, b, c, d = (Project(h, staged, n) for n in ("pa", "pb", "pc", "pd"))
        # (a) two at work, the third waits and is served when one falls idle; nothing in flight is lost
        a.work(60); b.work(60); time.sleep(6)
        ok("(a) two projects at work: each has its own headless KiCad", [n for _, n in h.servers()] == ["pa", "pb"], h.servers())
        t0 = time.time(); wait = later(c.version); time.sleep(4)
        ok("(a) the third project's request waits: nothing is stopped for it", "r" not in wait and [n for _, n in h.servers()] == ["pa", "pb"], h.servers())
        a.end(); freed = time.time(); r, e, s = got(wait)
        ok(f"(a) it is served once one has been idle {IN_USE} s", not e and IN_USE - 1.5 <= time.time() - freed <= IN_USE + 8, {"error": e, "after the other went idle": round(time.time() - freed, 1), "waited": round(s, 1)})
        b.end()
        ok("(a) nothing in flight was lost: every request of the two at work was answered", not a.errors and not b.errors and a.n > 3 and b.n > 3, {"pa": [a.n, a.errors[:2]], "pb": [b.n, b.errors[:2]]})
        for _ in range(100):   # (the one told to stop exits a moment later)
            if len(h.servers()) <= 2: break
            time.sleep(0.1)
        ok("(a) two servers, the idle one's place taken", [n for _, n in h.servers()] == ["pb", "pc"], h.servers())
        # (b) both at work past the room wait: the third is told so; its next call after one ends is answered
        b.work(ROOM_WAIT + 25); c.work(ROOM_WAIT + 25); time.sleep(2)
        r, e, s = a.version()
        ok(f"(b) both at work past {ROOM_WAIT} s: the third is told the pool is busy", "IPC busy: every KiCad server" in e and ROOM_WAIT - 1 <= s <= ROOM_WAIT + 6, {"error": e[:200], "after": round(s, 1)})
        c.end(); time.sleep(IN_USE + 1.5); r, e, s = a.version()
        ok("(b) its next call, after one has ended, is answered", not e and s < 25, {"error": e, "after": round(s, 1)})
        b.end(); ok("(b) nothing in flight was lost", not b.errors and not c.errors, {"pb": b.errors[:2], "pc": c.errors[:2]})
        # (c) a headless KiCad killed mid-request: its caller gets a plain error, its place is free, the project starts afresh
        r, e, s = b.read(); mine = h.servers("pb")
        ok("(c) before: one process for the project", len(mine) == 1 and not e, [mine, e])
        pid = mine[0][0]; before = h.socket_of(pid)
        burst = [later(b.read) for _ in range(60)]; time.sleep(0.05); os.kill(pid, signal.SIGKILL)   # the pid captured above, a child of this test's hub
        answers = [got(x, 60) for x in burst]; errs = [e for _, e, _ in answers if e]
        kinds = sorted({e[:110] for e in errs}); lost = [e for e in errs if "exited before answering" in e or "closed the connection" in e]
        # (those out at the kill say the server exited or closed the connection; those sent in the second before the hub's restart of it
        # say what they met: no server yet, or a new one with no document open)
        ok("(c) the requests out when it was killed are answered with a plain error, at once", lost and not any("panicked" in e or "did not answer in" in e for e in errs) and max(s for _, _, s in answers) < 10, {"kinds of error": kinds, "errors": len(errs), "answered": len(answers), "slowest": round(max(s for _, _, s in answers), 1)})
        # (a killed server is started again by the hub, as after any crash; it is idle, so with pa at work pd can have its place)
        b.doc = None; a.work(40); time.sleep(IN_USE + 3); r, e, s = d.version()
        ok("(c) the killed project counts as used by nobody: a further project gets a server", not e and s < 20, {"error": e, "after": round(s, 1), "servers": h.servers()})
        a.end(); time.sleep(IN_USE + 1.5); r, e, s = b.read(); now = h.servers("pb")
        ok("(c) the next request to the killed project is answered by a fresh process", not e and len(now) == 1 and now[0][0] != pid, {"error": e, "was": pid, "now": now})
        if len(now) == 1: after = h.socket_of(now[0][0]); ok("(c) on a replacement socket, with the request accepted by the new server", after and after != before, {"before": before, "after": after})
        # (e) a project asked for again while its last process is still exiting never has two processes
        seen, stop = {}, []
        def sample():
            while not stop:
                c0 = {}
                for _, n in h.servers(): c0[n] = c0.get(n, 0) + 1
                for n, k in c0.items(): seen[n] = max(seen.get(n, 0), k)
                seen["all"] = max(seen.get("all", 0), sum(c0.values())); time.sleep(0.02)
        st = threading.Thread(target=sample); st.start(); errors = []
        ring = [a, b, c, d]
        for i in range(8):
            time.sleep(IN_USE + 1.2)                       # the two servers are idle: the next two asks each take a place...
            x, y = ring[i % 4], ring[(i + 1) % 4]
            p, q = later(x.version), later(y.version); time.sleep(0.05)
            z = [later(w.version) for w in ring if w not in (x, y)]   # ...and the two just stopped are asked for again at once
            for box in [p, q] + z:
                r, e, s = got(box, 120)
                if e and "IPC busy" not in e: errors.append(e)
        stop.append(1); st.join()
        ok("(e) asked for again while the old process exits: never two processes for one project", all(seen.get(n, 0) <= 1 for n in ("pa", "pb", "pc", "pd")) and seen.get("all", 0) >= 2, seen)
        ok("(e) and no request failed for it (one told the pool is busy is not a failure)", not errors, errors[:3])
        for p in (a, b, c, d): p.release()
    finally: h.stop()

def second_hub(work, hub_bin, rt, staged, port):
    gib = int(next(l for l in open("/proc/meminfo") if l.startswith("MemTotal")).split()[1]) >> 20
    if gib < 30: print(f"skip (f): this machine reads {gib} GiB, the pool has room for two"); return
    h = Hub(work, hub_bin, rt, port, KNOBS); active = []
    try:
        ps = [Project(h, staged, n) for n in ("qa", "qb", "qc")]; first, starts = {}, {}
        for p in ps:
            r, e, s = p.version(); starts[p.name] = round(s, 2)
            if e: first[p.name] = e
            else: active.append(p.work(600, heartbeat=True))
        # Cold starts can exceed SPARE_IDLE under load. Keep the earlier servers active until all three
        # are ready, then stop together and establish the retirement order with warm requests.
        names = [n for _, n in h.servers()]
        for p in active: p.stop_at = 0
        for p in active:
            p.thread.join()
            if p.errors: first[p.name] = p.errors
        ok(f"(f) {gib} GiB: the third project has a server of its own beside the two", not first and names == ["qa", "qb", "qc"], {"errors": first, "servers": names, "cold_start_seconds": starts})
        for p in ps:
            r, e, s = p.version()
            if e: raise RuntimeError(f"could not establish idle order for {p.name}: {e}")
            time.sleep(1.2)
        deadline = time.time() + SPARE_IDLE + 75; left = h.servers()
        while time.time() < deadline and len(left) > 2: time.sleep(1); left = h.servers()
        ok(f"(f) idle {SPARE_IDLE} s, the one beyond two is retired: the least recently used", [n for _, n in left] == ["qb", "qc"], left)
        time.sleep(SPARE_IDLE + 5); ok("(f) the two stay", [n for _, n in h.servers()] == ["qb", "qc"], h.servers())
        r, e, s = ps[0].version(); ok("(f) and the retired project is served again when asked", not e and len(h.servers()) == 3, {"error": e, "servers": h.servers()})
        for p in ps: p.release()
    finally:
        for p in active: p.stop_at = 0
        for p in active: p.thread.join()
        h.stop()

if __name__ == "__main__":
    work, hub_bin, rt, board = [os.path.abspath(x) for x in sys.argv[1:5]]; port = int(sys.argv[5]) if len(sys.argv) > 5 else 3897
    os.makedirs(work, exist_ok=True); staged = work + "/staged"
    subprocess.run(["bash", "-c", f'. "{HERE}/../kicad_embed/lib/real_use.sh"; stage_board "$1" "$2"', "x", board, staged], check=True)
    print(f"pool e2e: hub {hub_bin}; runtime {subprocess.run([rt + '/bin/kicad-cli', '--version'], capture_output=True, text=True).stdout.strip()} at {rt}; board {os.path.basename(board)} (staged copy)", flush=True)
    first_hub(work + "/one", hub_bin, rt, staged, port)
    second_hub(work + "/two", hub_bin, rt, staged, port + 1)
    print(f"pool e2e: {sum(checks)} of {len(checks)}"); sys.exit(0 if all(checks) else 1)
