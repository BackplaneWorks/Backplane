"""The design tasks' checks (tasks2.json): check(task, run_dir, fixtures_dir, cli) -> [what is wrong]; empty is a pass.
Many results are right for routing and layout, so a run is held to rules, not to one reference: what KiCad's DRC and ERC
say (nothing new), the netlist, the spec's places and widths, and nothing else moved. Written before any run."""
import fnmatch, json, math, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import kx, norm

SILK = {"silk_over_copper", "silk_overlap", "silk_edge_clearance", "text_height", "text_thickness"}
def base(fx, name): return json.load(open(f"{fx}/{name}.base.json"))
def by_type(vs):
    c = {}
    for v in vs: c[v["type"]] = c.get(v["type"], 0) + 1
    return c
def more(now, before, skip=()):
    """violation kinds there are more of now than before"""
    a, b = by_type(now), by_type(before)
    return [f"DRC/ERC {k}: {a[k]} (was {b.get(k, 0)})" for k in sorted(a) if a[k] > b.get(k, 0) and k not in skip]
def moved(fps, before, keep):
    return [f"{r} moved or changed" for r, f in before.items() if r not in keep and (r not in fps or fps[r]["at"] != f["at"] or fps[r]["rot"] != f["rot"] or fps[r]["layer"] != f["layer"])]
def segs(pcb, nets, where=False):
    t = norm.parse(open(pcb).read())
    return [(s[0], norm.val(s, "net"), float(norm.val(s, "width") or 0)) + ((json.dumps(norm.kid(s, "start")), json.dumps(norm.kid(s, "end")), norm.val(s, "layer")) if where else ())
            for k in ("segment", "arc") for s in norm.kids(t, k) if norm.val(s, "net") in nets]
def near(a, b, tol): return math.dist(a, b) <= tol
def ongrid(v, g=1.27): return abs(v / g - round(v / g)) < 0.01

def sch_ldo(d, fx, cli):
    bad = []; sch = f"{d}/pic_programmer.kicad_sch"; b = base(fx, "pic")
    comps, nets = kx.netlist(cli, sch)
    want = {"U7": ("Regulator_Linear:AMS1117-3.3", "Package_TO_SOT_SMD:SOT-223-3_TabPin2", None),
            "C10": ("Device:C", "Capacitor_SMD:C_0805_2012Metric", "10u"), "C11": ("Device:C", "Capacitor_SMD:C_0805_2012Metric", "22u")}
    for r, (lib, fp, val) in want.items():
        c = comps.get(r)
        if not c: bad.append(f"{r} missing"); continue
        if c["lib"] != lib: bad.append(f"{r} symbol {c['lib']}")
        if c["footprint"] != fp: bad.append(f"{r} footprint {c['footprint']}")
        if val and not re.fullmatch(val + r"\s*[Fµu]?F?", c["value"].replace("µ", "u")): bad.append(f"{r} value {c['value']}")
    def pin(r, p): return comps.get(r, {}).get("pins", {}).get(p)
    if comps.get("U7"):
        if pin("U7", "3") != "VCC": bad.append(f"U7 VI on {pin('U7', '3')}")
        if pin("U7", "2") != "+3V3": bad.append(f"U7 VO on {pin('U7', '2')}")
        if pin("U7", "1") != "GND": bad.append(f"U7 GND on {pin('U7', '1')}")
    for r, n in (("C10", "VCC"), ("C11", "+3V3")):
        if comps.get(r) and sorted([pin(r, "1"), pin(r, "2")], key=str) != sorted([n, "GND"]): bad.append(f"{r} between {pin(r, '1')} and {pin(r, '2')}")
    bad += more(kx.erc(cli, sch), b["erc"])
    s = norm.sheet(sch); old = b["sheet"]
    for k, v in old["symbols"].items():
        if s["symbols"].get(k) != v: bad.append(f"symbol {k} changed")
    for w in json.loads(json.dumps(old["wires"])):
        if w not in s["wires"]: bad.append("a wire of the sheet changed"); break
    for k, v in s["symbols"].items():  # every new symbol in the area and on the grid
        if k in old["symbols"]: continue
        x, y = v["at"][:2]
        if not (190 <= x <= 280 and 20 <= y <= 60): bad.append(f"{k} at {x},{y}: outside the area")
        if not (ongrid(x) and ongrid(y)): bad.append(f"{k} at {x},{y}: off the grid")
    for w in s["wires"]:
        if w in old["wires"]: continue
        for x, y in json.loads(w):
            if not (185 <= x <= 285 and 15 <= y <= 65): bad.append(f"a new wire at {x},{y}: outside the area"); break
    return bad

LED = [f"/LED{i}" for i in range(1, 8)]
def route_leds(d, fx, cli):
    bad = []; pcb = f"{d}/StickHub.kicad_pcb"; b = base(fx, "sh_route")
    pro = json.load(open(f"{d}/StickHub.kicad_pro"))["net_settings"]
    cl = [c for c in pro["classes"] if c["name"] == "LED"]
    if not cl: bad.append("no net class LED")
    elif abs(cl[0].get("track_width", 0) - 0.2) > 1e-6: bad.append(f"LED class width {cl[0].get('track_width')}")
    pats = [p["pattern"] for p in pro.get("netclass_patterns") or [] if p["netclass"] == "LED"]
    for n in LED:
        if not any(fnmatch.fnmatchcase(n, p) for p in pats): bad.append(f"{n} not in class LED"); break
    ss = segs(pcb, set(LED))
    for k, n, w in ss:
        if abs(w - 0.2) > 0.001: bad.append(f"{n} {k} {w} mm wide"); break
    for n in LED:
        if not any(x[1] == n for x in ss): bad.append(f"{n} has no copper")
    r = kx.drc(cli, pcb)
    if r["unconnected_items"]: bad.append(f"{len(r['unconnected_items'])} unconnected: " + "; ".join(kx.nettag(u["items"][0]["description"]) for u in r["unconnected_items"][:4]))
    bad += [f"DRC {v['type']}: {v['description'][:60]}" for v in r["violations"]][:6]
    # the pours: as saved, they are what a refill gives
    t = norm.board(pcb)["zones"]; import shutil, tempfile
    with tempfile.TemporaryDirectory() as tmp:
        for f in os.listdir(d):
            if f.startswith("StickHub."): shutil.copy(f"{d}/{f}", tmp)
        kx.run(cli, "pcb", "drc", "--refill-zones", "--save-board", "-o", tmp + "/r.rpt", tmp + "/StickHub.kicad_pcb")
        t2 = norm.board(tmp + "/StickHub.kicad_pcb")["zones"]
    a = sorted((json.loads(z)["net"], json.loads(z)["fill_mm2"]) for z in t); a2 = sorted((json.loads(z)["net"], json.loads(z)["fill_mm2"]) for z in t2)
    for (n, x), (_, y) in zip(a, a2):
        if abs(x - y) > max(0.5, 0.005 * y): bad.append(f"zone {n} not refilled: {x} mm2, a refill gives {y}")
    bad += moved(norm.board(pcb)["footprints"], b["footprints"], ())
    return bad

def place_decap(d, fx, cli):
    bad = []; pcb = f"{d}/StickHub.kicad_pcb"; b = base(fx, "sh_place"); spec = json.load(open(f"{fx}/sh_place.spec.json"))
    fps = norm.board(pcb)["footprints"]; P = kx.pads(cli, pcb)
    t = norm.parse(open(pcb).read()); padnet = {}
    for f in norm.kids(t, "footprint"):
        for p in norm.kids(f, "pad"): n = norm.kid(p, "net"); padnet[(norm.props(f).get("Reference"), p[1])] = n[-1] if n else ""
    for c, s in spec.items():
        if fps[c]["layer"] != "B.Cu": bad.append(f"{c} on {fps[c]['layer']}")
        pn = [p for p in ("1", "2") if padnet.get((c, p)) == s["net"]]
        if not pn or (c, pn[0]) not in P or ("U1", s["pin"]) not in P: bad.append(f"{c}: its {s['net']} pad not found"); continue
        dist = math.dist(P[(c, pn[0])], P[("U1", s["pin"])])
        if dist > 2.0: bad.append(f"{c}: {dist:.2f} mm from U1 pin {s['pin']}")
    r = kx.drc(cli, pcb)
    caps = set(spec)
    for v in r["violations"]:
        if v["type"] in SILK or v["type"] in ("track_dangling", "via_dangling"): continue
        if any(re.search(r"of (%s)\b" % "|".join(caps), i["description"]) for i in v["items"]): bad.append(f"DRC {v['type']}: {v['description'][:50]} " + " / ".join(i["description"][:40] for i in v["items"]))
    bad += more(r["violations"], b["drc"]["violations"], skip=SILK | {"track_dangling", "via_dangling", "copper_outside_board"})
    bad += moved(fps, b["footprints"], caps)
    return bad[:10]

SWAP = {**{f"R{i}": "Resistor_THT:R_Axial_DIN0207_L6.3mm_D2.5mm_P7.62mm_Horizontal" for i in range(1, 8)},
        "C6": "Capacitor_THT:C_Disc_D3.0mm_W1.6mm_P2.50mm", "C7": "Capacitor_THT:C_Disc_D3.0mm_W1.6mm_P2.50mm"}
def swap_fp(d, fx, cli):
    bad = []; pcb = f"{d}/pic_programmer.kicad_pcb"; sch = f"{d}/pic_programmer.kicad_sch"; b = base(fx, "pic")
    fps = norm.board(pcb)["footprints"]; P = kx.pads(cli, pcb)
    comps, _ = kx.netlist(cli, sch)
    for r, lib in SWAP.items():
        if fps.get(r, {}).get("lib") != lib: bad.append(f"{r} board footprint {fps.get(r, {}).get('lib')}")
        if comps.get(r, {}).get("footprint") != lib: bad.append(f"{r} schematic footprint {comps.get(r, {}).get('footprint')}")
        if r in fps and fps[r]["rot"] != b["footprints"][r]["rot"]: bad.append(f"{r} rotated")
        if (r, "1") in P and not near(P[(r, "1")], b["pads"][f"{r}.1"], 0.05): bad.append(f"{r} pad 1 moved")
    t = norm.parse(open(pcb).read()); old = {}
    for f in norm.kids(t, "footprint"):
        ref = norm.props(f).get("Reference")
        if ref in SWAP:
            ns = sorted((p[1], (norm.kid(p, "net") or [""])[-1]) for p in norm.kids(f, "pad"))
            want = sorted((p, comps[ref]["pins"].get(p, "")) for p in ("1", "2")) if ref in comps else None
            if want and [n for _, n in ns] != [n for _, n in want]: bad.append(f"{ref} pads' nets {ns}")
    r = kx.drc(cli, pcb)
    if r["unconnected_items"]: bad.append(f"{len(r['unconnected_items'])} unconnected: " + "; ".join(kx.nettag(u["items"][0]["description"]) for u in r["unconnected_items"][:4]))
    bad += more(r["violations"], b["drc"]["violations"])
    bad += moved(fps, b["footprints"], set(SWAP))
    s = norm.sheet(sch)
    for k, v in b["sheet"]["symbols"].items():
        if k.split(":")[0] in SWAP: continue
        if s["symbols"].get(k) != v: bad.append(f"symbol {k} changed"); break
    return bad[:10]

DRV = [f"Net-(D{i}-1)" for i in range(15, 22)]
def route_drv(d, fx, cli):
    """the head-to-head's third routing task: seven three-pad nets across the board, bottom to top"""
    bad = []; pcb = f"{d}/StickHub.kicad_pcb"; b = base(fx, "sh_drv")
    ss = segs(pcb, set(DRV))
    for k, n, w in ss:
        if w < 0.15 - 0.001: bad.append(f"{n} {k} {w} mm wide"); break
    for n in DRV:
        if not any(x[1] == n for x in ss): bad.append(f"{n} has no copper")
    r = kx.drc(cli, pcb)
    if r["unconnected_items"]: bad.append(f"{len(r['unconnected_items'])} unconnected: " + "; ".join(kx.nettag(u["items"][0]["description"]) for u in r["unconnected_items"][:4]))
    bad += [f"DRC {v['type']}: {v['description'][:60]}" for v in r["violations"]][:6]
    bad += moved(norm.board(pcb)["footprints"], b["footprints"], ())
    return bad

HNETS = ["Net-(U9-B1)", "Net-(U9-B2)", "Net-(U9-B3)", "/Debugger/DBG_LS_OE"]
def hubble_route(d, fx, cli):
    bad = []; pcb = f"{d}/Hubble_DK.kicad_pcb"; b = base(fx, "hubble")
    r = kx.drc(cli, pcb)
    left = [u for u in r["unconnected_items"] if kx.nettag(u["items"][0]["description"]) in HNETS]
    if left: bad.append(f"{len(left)} still unconnected: " + "; ".join(kx.nettag(u["items"][0]["description"]) for u in left))
    key = lambda v: (v["type"], tuple(sorted(i["description"] for i in v["items"])))
    was = {key(v) for v in b["drc"]["violations"]}
    for v in r["violations"]:  # nothing new on these nets (what was there before, a stub's dangling end, stays)
        if v["type"] in SILK or key(v) in was: continue
        if any(kx.nettag(i["description"]) in HNETS for i in v["items"]): bad.append(f"DRC {v['type']} on " + " / ".join(i["description"][:45] for i in v["items"]))
    bad += more(r["violations"], b["drc"]["violations"])
    old = set(segs(f"{fx}/hubble/Hubble_DK.kicad_pcb", set(HNETS), True))
    for k, n, w, *_ in segs(pcb, set(HNETS), True):  # the new copper at the net class's width (0.1 mm) or wider
        if (k, n, w, *_) not in old and w < 0.1 - 1e-6: bad.append(f"{n}: a new {k} {w} mm wide (class 0.1)"); break
    bad += moved(norm.board(pcb)["footprints"], b["footprints"], ())
    return bad[:10]

def check(task, d, fx, cli): return globals()[task](d, fx, cli)
if __name__ == "__main__":  # checks.py TASK RUN_DIR FIXTURES CLI
    print(json.dumps(check(*sys.argv[1:5]), indent=1))
