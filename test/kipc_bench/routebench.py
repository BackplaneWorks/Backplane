#!/usr/bin/env python3
"""The routing benchmark (docs/kicad-route-bench.md): the fork's SketchRoute against the hub's router, on fixed boards, no model.
  routebench.py mk H2H_FX HUBBLE_DIR COLLAR_DIR CLI OUT     the cases (copies only): OUT/<case>/ and OUT/cases.json, with what DRC said before
  routebench.py sketch CLI PROTO_DIR CASES OUT [--cases a,b] [--reps N] [--per-net] [--neck] [--time-limit MS]
                                                            each case routed by SketchRoute over IPC on a headless `kicad-cli api-server`
  routebench.py hub MCP_URL CASES OUT [--cases a,b] [--reps N]   each case routed by kicad_edit's `route` on a benchmark hub (hub.mjs)
  routebench.py score CLI CASES OUT                         OUT/score.json and a table: nets done, vias, length against the shortest
                                                            tree, DRC findings the routes added, clearance over the rule, seconds
A case's nets were routed on the real board and are taken up, so every case has a known answer. Times are the routing call alone (no
DRC run): SketchRoute's wall time and its own `elapsed_ms`; the hub's `kicad_edit` with check: false, which still opens, refills and saves."""
import argparse, json, math, os, re, shutil, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import kx, norm, h2h

HUBBLE_DENSE = r"^/(A\d|D\d+|Debugger/.*|FEM\..*|I2S\..*|IRQ\..*|MIC\..*|PDM\..*|PMIC\..*|SPI\..*|SWD\..*|UART\..*|nRESET|SHPHLD)$"
COLLAR_DENSE = r"^/(I2C_.*|IMU_.*|LED_.*|MIC_.*|PDM_.*|PMIC_INT|SHPHLD|SWD.*|USB_CC\d|VSET1|~\{RESET\})$"
SKIP = ("history.git", ".backplane", "*.lck", "*-backups", "build", "_autosave*", "backups", "mfg", "docs", "scripts")

def take_up(pcb, pat):
    """the copper (tracks, arcs, vias) of the nets matching pat is removed; the nets, sorted"""
    t = open(pcb).read(); rx = re.compile(pat); got = set(); keep = []; last = 0
    for s, e, h in kx.blocks(t):
        if h not in ("segment", "arc", "via"): continue
        m = re.search(r'\(net "((?:[^"\\]|\\.)*)"\)', t[s:e])
        if m and rx.search(m.group(1)): got.add(m.group(1)); keep.append(t[last:s]); last = e
    keep.append(t[last:]); open(pcb, "w").write("".join(keep)); return sorted(got)
def clearance(pro):
    c = json.load(open(pro))["net_settings"]["classes"]; return next(x["clearance"] for x in c if x["name"] == "Default")
def base(cli, d, name):
    r = kx.drc(cli, f"{d}/{name}.kicad_pcb", refill=True)
    return {"violations": len(r["violations"]), "by_type": count(v["type"] for v in r["violations"]), "unconnected": len(r["unconnected_items"])}
def count(it):
    o = {}
    for x in it: o[x] = o.get(x, 0) + 1
    return o

def mk(fx, hubble, collar, cli, out):
    shutil.rmtree(out, ignore_errors=True); os.makedirs(out); cases = []
    def case(cid, src, name, nets=None, pat=None, rule=None):
        d = f"{out}/{cid}"; shutil.copytree(src, d, ignore=shutil.ignore_patterns(*SKIP))
        if pat: nets = take_up(f"{d}/{name}.kicad_pcb", pat)
        c = {"id": cid, "name": name, "nets": nets, "rule": rule or clearance(f"{d}/{name}.kicad_pro")}; cases.append(c); return d
    d = case("route_leds", f"{fx}/sh_route", "StickHub", [f"/LED{i}" for i in range(1, 8)], rule=0.15)
    # the task's net class is part of the fixture here, so both routers get the same board (the task's own first step)
    pro = f"{d}/StickHub.kicad_pro"; p = json.load(open(pro)); ns = p["net_settings"]; led = dict(ns["classes"][0]); led.update(name="LED", track_width=0.2, clearance=0.15, priority=0)
    ns["classes"].append(led); ns["netclass_patterns"] = (ns.get("netclass_patterns") or []) + [{"netclass": "LED", "pattern": "/LED*"}]; json.dump(p, open(pro, "w"), indent=2)
    case("route_drv", f"{fx}/sh_drv", "StickHub", [f"Net-(D{i}-1)" for i in range(15, 22)], rule=0.15)
    # the whole demo board routed again: every net's copper taken up but the pours' own nets (GND and the supplies they carry)
    t = open(f"{fx}/sh_drv/StickHub.kicad_pcb").read(); zoned = set(re.findall(r'\(zone\b[^()]*(?:\([^()]*\)[^()]*)*?\(net_name "((?:[^"\\]|\\.)*)"\)', t)) | set(re.findall(r'\(zone\s+\(net "((?:[^"\\]|\\.)*)"\)', t))
    every = sorted(n for n in set(re.findall(r'\((?:segment|via|arc)\b[^()]*(?:\([^()]*\)[^()]*)*\(net "((?:[^"\\]|\\.)*)"\)', t)) | {f"Net-(D{i}-1)" for i in range(15, 22)} if n and n not in zoned)
    case("stickhub_all", f"{fx}/sh_drv", "StickHub", pat="^(" + "|".join(map(re.escape, every)) + ")$", rule=0.15); cases[-1]["nets"] = every
    case("hubble_route", f"{fx}/hubble", "Hubble_DK", ["Net-(U9-B1)", "Net-(U9-B2)", "Net-(U9-B3)", "/Debugger/DBG_LS_OE"])
    # Hubble_DK is part way through routing: `hubble_known` takes up the nets its designer finished (a known answer); `hubble_dense` the
    # 49 signal nets of the pattern, of which the designer finished one (the rest have escape stubs only): what a router can add, no known answer
    r = kx.drc(cli, f"{hubble}/Hubble_DK.kicad_pcb", refill=True); unfinished = {kx.nettag(i["description"]) for u in r["unconnected_items"] for i in u["items"]}
    t = open(f"{hubble}/Hubble_DK.kicad_pcb").read(); zoned = set(re.findall(r'\(zone\b[^()]*(?:\([^()]*\)[^()]*)*?\(net_name "((?:[^"\\]|\\.)*)"\)', t)) | set(re.findall(r'\(zone\s+\(net "((?:[^"\\]|\\.)*)"\)', t))
    finished = sorted(n for n in set(re.findall(r'\((?:segment|via|arc)\b[^()]*(?:\([^()]*\)[^()]*)*\(net "((?:[^"\\]|\\.)*)"\)', t)) if n not in unfinished and n not in zoned and not n.startswith("unconnected-"))
    case("hubble_known", hubble, "Hubble_DK", pat="^(" + "|".join(map(re.escape, finished)) + ")$")
    case("hubble_dense", hubble, "Hubble_DK", pat=HUBBLE_DENSE)
    case("collar_dense", collar, "collar_main", pat=COLLAR_DENSE)
    for c in cases:
        c["base"] = base(cli, f"{out}/{c['id']}", c["name"]); P = kx.pads(cli, f"{out}/{c['id']}/{c['name']}.kicad_pcb")
        _, _, padnet = h2h.copper(f"{out}/{c['id']}/{c['name']}.kicad_pcb", set(c["nets"]))
        c["shortest_tree"] = round(sum(h2h.mst([P[k] for k, n in padnet.items() if n == net and k in P]) for net in c["nets"]), 2)
        print(c["id"], len(c["nets"]), "nets", c["base"], "tree", c["shortest_tree"], flush=True)
    json.dump(cases, open(f"{out}/cases.json", "w"), indent=1)

def fresh(cases, c, out, tag, rep=0):
    d = f"{out}/{tag}/{c['id']}" + (f".{rep}" if rep else ""); shutil.rmtree(d, ignore_errors=True); shutil.copytree(f"{cases}/{c['id']}", d); return d

class Ipc:
    """a headless api-server with its own HOME, and KiCad's protobuf messages"""
    def __init__(self, cli, proto, env=None):
        import pynng
        self.root = tempfile.mkdtemp(prefix="routebench-", dir=os.environ.get("BP_TMP") or None); b = f"{self.root}/b"; os.makedirs(b)
        files = [os.path.join(dp, f) for dp, _, fs in os.walk(proto) for f in fs if f.endswith(".proto")]
        subprocess.run(["protoc", f"-I{proto}", f"--python_out={b}", *sorted(files)], check=True); sys.path.insert(0, b)
        e = dict(os.environ); e.update(env or {})
        for v, s in (("HOME", "home"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache"), ("XDG_CONFIG_HOME", "config"), ("KICAD_CONFIG_HOME", "config/kicad")):
            os.makedirs(f"{self.root}/{s}", exist_ok=True); e[v] = f"{self.root}/{s}"
        e.pop("DISPLAY", None); e.pop("WAYLAND_DISPLAY", None); self.sock = f"{self.root}/api.sock"
        pre = os.environ.get("ROUTEBENCH_WRAP", "").split()  # e.g. `perf record -g -o FILE --`
        self.srv = subprocess.Popen([*pre, cli, "api-server", "--socket", self.sock], env=e, stdout=open(f"{self.root}/server.log", "w"), stderr=subprocess.STDOUT)
        t = time.monotonic()
        while not os.path.exists(self.sock):
            if self.srv.poll() is not None or time.monotonic() - t > 60: raise RuntimeError("api-server did not start: " + open(f"{self.root}/server.log").read()[-2000:])
            time.sleep(0.05)
        self.conn = pynng.Req0(dial=f"ipc://{self.sock}", block_on_dial=True, send_timeout=900000, recv_timeout=900000); self.token = ""
        import importlib; m = lambda n: importlib.import_module(n + "_pb2")
        self.env_, self.project, self.editor, self.sketch, self.board, self.types, self.enums, self.bcmd = m("common.envelope"), m("common.commands.project_commands"), m("common.commands.editor_commands"), m("backplane.sketch_route"), m("board.board_types"), m("common.types.base_types"), m("common.types.enums"), m("board.board_commands")
    def req(self, cmd, ty=None):
        r = self.env_.ApiRequest(); r.header.client_name = "works.backplane.routebench"; r.header.kicad_token = self.token; r.message.Pack(cmd)
        self.conn.send(r.SerializeToString()); a = self.env_.ApiResponse.FromString(self.conn.recv()); self.token = a.header.kicad_token
        if a.status.status != self.env_.AS_OK: raise RuntimeError(f"{cmd.DESCRIPTOR.name}: {a.status}")
        if ty is None: return None
        o = ty(); a.message.Unpack(o); return o
    def close(self):
        try: self.conn.close()
        except Exception: pass
        self.srv.terminate()
        try: self.srv.wait(20)
        except Exception: self.srv.kill()
        (shutil.copy(f"{self.root}/server.log", os.environ["ROUTEBENCH_LOG"]) if os.environ.get("ROUTEBENCH_LOG") else None); shutil.rmtree(self.root, ignore_errors=True)

def sketch_case(cli, proto, cases, c, out, tag, per_net=False, limit=0, neck=False, rep=0):
    """neck: a connection with no way at its width may be as narrow as the board's own least track width"""
    d = fresh(cases, c, out, tag, rep); pcb = f"{d}/{c['name']}.kicad_pcb"; k = Ipc(cli, proto); res = {"case": c["id"], "router": "sketch", "dir": d}
    try:
        t0 = time.monotonic(); doc = k.req(k.project.OpenDocument(type=k.types.DOCTYPE_PCB, path=pcb), k.project.OpenDocumentResponse).document; res["open_s"] = round(time.monotonic() - t0, 3)
        hdr = k.types.ItemHeader(document=doc); bynet = {}
        for kind, msg in ((k.enums.KOT_PCB_PAD, k.board.Pad), (k.enums.KOT_PCB_TRACE, k.board.Track), (k.enums.KOT_PCB_ARC, k.board.Arc), (k.enums.KOT_PCB_VIA, k.board.Via)):
            for packed in k.req(k.editor.GetItems(header=hdr, types=[kind]), k.editor.GetItemsResponse).items:
                it = msg(); packed.Unpack(it)
                if it.net.name in c["nets"]: bynet.setdefault(it.net.name, []).append(it.id)
        groups = [[n] for n in c["nets"]] if per_net else [c["nets"]]; calls = []; tot = {"connections": 0, "routed": 0, "failed": 0, "vias": 0, "narrowed": 0, "elapsed_ms": 0.0, "failures": [], "timed_out": False}
        extra = {"min_track_width": k.types.Distance(value_nm=int(round(json.load(open(f"{d}/{c['name']}.kicad_pro"))["board"]["design_settings"]["rules"]["min_track_width"] * 1e6)))} if neck else {}
        t0 = time.monotonic()
        for g in groups:
            ids = [i for n in g for i in bynet.get(n, [])]; t1 = time.monotonic()
            r = k.req(k.sketch.SketchRoute(board=doc, items=ids, time_limit_ms=limit, **extra), k.sketch.SketchRouteResponse); calls.append(round(time.monotonic() - t1, 4))
            for f in ("connections", "routed", "failed", "vias", "elapsed_ms"): tot[f] += getattr(r, f)
            tot["narrowed"] += getattr(r, "narrowed", 0)
            tot["timed_out"] |= r.timed_out; tot["failures"] += [{"net": f.net, "reason": f.reason} for f in r.failures]
        res.update(tot); res["route_s"] = round(time.monotonic() - t0, 4); res["calls_s"] = calls
        t0 = time.monotonic(); k.req(k.bcmd.RefillZones(board=doc)); k.req(k.project.SaveDocument(document=doc)); res["refill_save_s"] = round(time.monotonic() - t0, 3)
    finally: k.close()
    return res

def hub_case(url, cases, c, out, tag, rep=0):
    """each repeat is a folder of its own: the hub's KiCad keeps a board it has opened"""
    import urllib.request
    from mcp import Mcp
    class Long(Mcp):
        def raw(self, method, params):
            self.n += 1; q = urllib.request.Request(self.url, json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}).encode(), {"content-type": "application/json", "accept": "application/json"})
            return json.loads(urllib.request.urlopen(q, timeout=7200).read())
    d = fresh(cases, c, out, tag, rep); m = Long(url); t0 = time.monotonic()
    r = m.call("kicad_edit", {"path": f"{d}/{c['name']}.kicad_pcb", "ops": [{"route": c["nets"]}], "message": "routebench", "show": False, "check": False})
    rr = (r.get("results") or [{}])[0]; rows = rr.get("routed") or rr.get("nets") or rr
    return {"case": c["id"], "router": "hub", "dir": d, "route_s": round(time.monotonic() - t0, 3), "answer": rows, "saved": r.get("saved"), "not_saved": r.get("not_saved")}

def score_case(cli, cases, c, d):
    name, nets = c["name"], set(c["nets"]); pcb = f"{d}/{name}.kicad_pcb"; src = f"{cases}/{c['id']}/{name}.kicad_pcb"
    segs, vias, padnet = h2h.copper(pcb, nets); s0, v0, _ = h2h.copper(src, nets); P = kx.pads(cli, pcb)
    old = {(n, p, q, l) for n, p, q, w, l, k in s0}; new = [s for s in segs if (s[0], s[1], s[2], s[4]) not in old]; oldv = {(x[0], x[1]) for x in v0}; newv = [v for v in vias if (v[0], v[1]) not in oldv]
    r = kx.drc(cli, pcb, refill=True); on = lambda v: any(kx.nettag(i["description"]) in nets for i in v["items"])
    unc = {kx.nettag(i["description"]) for u in r["unconnected_items"] for i in u["items"]} & nets
    by = count(v["type"] for v in r["violations"]); added = {t: n - c["base"]["by_type"].get(t, 0) for t, n in by.items() if n > c["base"]["by_type"].get(t, 0)}
    done = [n for n in c["nets"] if n not in unc]; L = sum(math.dist(p, q) for n, p, q, *_ in segs if n in done)
    lb = sum(h2h.mst([P[k] for k, nn in padnet.items() if nn == n and k in P]) for n in done); b, a = h2h.bends(new)
    return {"nets": len(c["nets"]), "done": len(done), "unrouted": sorted(unc), "vias": len(newv), "via_kinds": count(v[4] for v in newv), "layers": sorted({s[4] for s in new}), "length": round(L, 2), "ratio": round(L / lb, 3) if lb else None,
            "bends": b, "acute": a, "drc_added": added, "drc_added_n": sum(added.values()), "drc_on_nets": count(v["type"] for v in r["violations"] if on(v)),
            "unconnected_elsewhere": max(0, len(r["unconnected_items"]) - c["base"]["unconnected"] - sum(1 for u in r["unconnected_items"] if on(u))),
            "margin": h2h.margin(d, name, list(nets), c["rule"], cli) if not unc or len(done) else None}

def table(rows):
    h = ["case", "router", "done", "vias", "ratio", "drc+", "on nets", "elsewhere", "margin", "route s", "s/net", "own ms"]
    out = [h] + [[r["case"], r["tag"], f"{r['done']}/{r['nets']}", str(r["vias"]), str(r["ratio"]), str(r["drc_added_n"]), ",".join(f"{k}:{v}" for k, v in r["drc_on_nets"].items()) or "-", str(r["unconnected_elsewhere"]), str(r["margin"]),
                  f"{r['route_s']:.3f}", f"{r['route_s'] / r['nets']:.4f}", f"{r['elapsed_ms']:.0f}" if "elapsed_ms" in r else "-"] for r in rows]
    w = [max(len(x[i]) for x in out) for i in range(len(h))]
    return "\n".join("  ".join(x[i].ljust(w[i]) for i in range(len(h))) for x in out)

if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "mk": mk(*sys.argv[2:7]); sys.exit()
    ap = argparse.ArgumentParser(); ap.add_argument("cmd"); ap.add_argument("a", nargs="+"); ap.add_argument("--cases"); ap.add_argument("--reps", type=int, default=1); ap.add_argument("--per-net", action="store_true"); ap.add_argument("--neck", action="store_true")
    ap.add_argument("--time-limit", type=int, default=0); ap.add_argument("--tag"); ap.add_argument("--tags"); a = ap.parse_args()
    cases_dir, out = a.a[-2], a.a[-1]; cases = [c for c in json.load(open(f"{cases_dir}/cases.json")) if not a.cases or c["id"] in a.cases.split(",")]
    if cmd in ("sketch", "hub"):
        tag = a.tag or cmd; os.makedirs(f"{out}/{tag}", exist_ok=True); rows = []
        for c in cases:
            best = None
            for i in range(a.reps):  # the fastest of the repeats (the result is the same each time: both routers are deterministic)
                r = sketch_case(a.a[0], a.a[1], cases_dir, c, out, tag, a.per_net, a.time_limit, a.neck, i) if cmd == "sketch" else hub_case(a.a[0], cases_dir, c, out, tag, i)
                if best is None or r["route_s"] < best["route_s"]: best = r
            rows.append(best); print(json.dumps({k: v for k, v in best.items() if k not in ("answer", "calls_s")}), flush=True)
        json.dump(rows, open(f"{out}/{tag}/run.json", "w"), indent=1)
    elif cmd == "score":
        rows = []
        for tag in (a.tags.split(",") if a.tags else sorted(t for t in os.listdir(out) if os.path.exists(f"{out}/{t}/run.json"))):
            for r in json.load(open(f"{out}/{tag}/run.json")):
                c = next((x for x in cases if x["id"] == r["case"]), None)
                if not c: continue
                s = score_case(a.a[0], cases_dir, c, r.get("dir") or f"{out}/{tag}/{c['id']}"); s.update({k: v for k, v in r.items() if k != "answer"}); s["tag"] = tag; rows.append(s)
        rows.sort(key=lambda r: ([c["id"] for c in cases].index(r["case"]), r["tag"])); json.dump(rows, open(f"{out}/score.json", "w"), indent=1); print(table(rows))
