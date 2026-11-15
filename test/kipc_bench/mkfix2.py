#!/usr/bin/env python3
"""The design tasks' fixtures (tasks2.json, checks.py): mkfix2.py CLI FIXTURES HUBBLE_DIR OUT
FIXTURES is mkfix.sh's output (stickhub, pic_programmer); HUBBLE_DIR a copy source of the Hubble_DK project (read only: it is copied).
  OUT/pic          pic_programmer as is (an LDO to add to the schematic; footprints to swap)
  OUT/sh_route     StickHub with the seven LED nets' copper taken up (to route again)
  OUT/sh_place     StickHub with eight decoupling caps moved off the board (to place again by U1's pins)
  OUT/hubble       Hubble_DK, a real 8-layer board part way through routing (four debugger nets to route)
and OUT/<fixture>.base.json: what KiCad says about each before the edit (DRC, ERC, pads, footprints), which the checks compare with."""
import json, os, re, shutil, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import kx, norm
cli, fx, hub, out = sys.argv[1:5]
shutil.rmtree(out, ignore_errors=True); os.makedirs(out)
shutil.copytree(f"{fx}/pic_programmer", f"{out}/pic")
LED = [f"/LED{i}" for i in range(1, 8)]
CAPS = ["C5", "C6", "C7", "C8", "C9", "C10", "C12", "C13"]

def edit(path, fn):
    t = open(path).read(); keep = []; last = 0
    for s, e, h in kx.blocks(t):
        r = fn(h, t[s:e])
        if r is not None: keep.append(t[last:s] + r); last = e
    keep.append(t[last:]); open(path, "w").write("".join(keep))

# routing: the LED nets' tracks, arcs and vias taken up
shutil.copytree(f"{fx}/stickhub", f"{out}/sh_route")
edit(f"{out}/sh_route/StickHub.kicad_pcb", lambda h, b: "" if h in ("segment", "arc", "via") and re.search(r'\(net "(%s)"\)' % "|".join(map(re.escape, LED)), b) else None)

# placement: each cap's nearest U1 pad of its own net (the spec), then the caps moved to a row off the board's right edge
shutil.copytree(f"{fx}/stickhub", f"{out}/sh_place")
pcb = f"{out}/sh_place/StickHub.kicad_pcb"; P = kx.pads(cli, pcb); _, nets = None, None
t = norm.parse(open(pcb).read()); padnet = {}
for f in norm.kids(t, "footprint"):
    ref = norm.props(f).get("Reference")
    for p in norm.kids(f, "pad"):
        n = norm.kid(p, "net"); padnet[(ref, p[1])] = n[-1] if n else ""
import math
spec = {}
for c in CAPS:
    best = None
    for pn in ("1", "2"):
        if padnet.get((c, pn)) == "GND": continue
        for (r, q), xy in P.items():
            if r == "U1" and padnet.get((r, q)) == padnet[(c, pn)]:
                d = math.dist(P[(c, pn)], xy)
                if best is None or d < best[0]: best = (d, q, padnet[(c, pn)])
    spec[c] = {"pin": best[1], "net": best[2], "orig_dist": round(best[0], 3)}
def park(h, b):
    if h != "footprint": return None
    m = re.search(r'\(property "Reference" "([^"]+)"', b)
    if not m or m.group(1) not in CAPS: return None
    i = CAPS.index(m.group(1))
    return re.sub(r"\(translate [-\d.]+ [-\d.]+\)", f"(translate {172 + 3 * (i % 4)} {90 + 4 * (i // 4)})", b, count=1)
edit(pcb, park)
json.dump(spec, open(f"{out}/sh_place.spec.json", "w"), indent=1)

# scale: Hubble_DK as it is (part way routed)
os.makedirs(f"{out}/hubble")
for n in ("Hubble_DK.kicad_pcb", "Hubble_DK.kicad_pro", "Hubble_DK.kicad_sch", "Hubble_DK.kicad_dru", "fp-lib-table", "sym-lib-table"):
    shutil.copy(f"{hub}/{n}", f"{out}/hubble/{n}")
for d in ("lib", "sheets"): shutil.copytree(f"{hub}/{d}", f"{out}/hubble/{d}")
kx.run(cli, "pcb", "upgrade", "--force", f"{out}/hubble/Hubble_DK.kicad_pcb")

# what KiCad says before the edit
for name, pcb, sch in (("pic", "pic/pic_programmer.kicad_pcb", "pic/pic_programmer.kicad_sch"), ("sh_route", "sh_route/StickHub.kicad_pcb", None),
                       ("sh_place", "sh_place/StickHub.kicad_pcb", None), ("hubble", "hubble/Hubble_DK.kicad_pcb", None)):
    b = {"drc": kx.drc(cli, f"{out}/{pcb}"), "pads": {f"{r}.{p}": xy for (r, p), xy in kx.pads(cli, f"{out}/{pcb}").items()},
         "footprints": norm.board(f"{out}/{pcb}")["footprints"]}
    if sch: b["erc"] = kx.erc(cli, f"{out}/{sch}"); b["sheet"] = norm.sheet(f"{out}/{sch}")
    json.dump(b, open(f"{out}/{name}.base.json", "w"))
    print(name, "drc", len(b["drc"]["violations"]), "unconnected", len(b["drc"]["unconnected_items"]), "erc", len(b.get("erc", [])))
print(json.dumps(spec))
