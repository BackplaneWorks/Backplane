#!/usr/bin/env python3
"""plot_bench.py OUT.json ORACLE BPPLOT [N]: parse+plot wall time (spawn to exit, info file written, stdout to a file) and peak RSS (wait4) of the Bend child and bp-plot,
N samples each, interleaved, on ecc83 and cm5_minima boards and schematics, then bp-plot alone on the two boards the child cannot read under the cap."""
import json, os, statistics as st, subprocess, sys, time
D = os.path.expanduser("~/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos")
out, oracle, rust = sys.argv[1:4]; n = int(sys.argv[4]) if len(sys.argv) > 4 else 20
W = os.path.abspath("build/gpui/files-run/plotport/bench"); os.makedirs(W, exist_ok=True)
def one(w):
    t = time.perf_counter()
    with open(f"{W}/o", "wb") as o: p = subprocess.Popen(w, stdin=subprocess.DEVNULL, stdout=o, stderr=subprocess.DEVNULL)
    _, s, ru = os.wait4(p.pid, 0)
    assert os.waitstatus_to_exitcode(s) == 0, w
    return (time.perf_counter() - t) * 1000, ru.ru_maxrss / 1024
def stats(v):
    v = sorted(v); return {"median": round(st.median(v), 1), "p95": round(v[min(len(v) - 1, int(0.95 * len(v)))], 1), "n": len(v)}
res = []
for rel, k in (("ecc83/ecc83-pp.kicad_pcb", 0), ("ecc83/ecc83-pp.kicad_sch", 1), ("cm5_minima/CM5_MINIMA_3.kicad_pcb", 0), ("cm5_minima/CM5_MINIMA_3.kicad_sch", 1), ("cm5_minima/USB.kicad_sch", 1), ("video/video.kicad_pcb", 0)):
    f = f"{D}/{rel}"; a = []; b = []
    for i in range(n):
        a.append(one([oracle, "--plot", str(k), f, f"{W}/a.infos"])); b.append(one([rust, str(k), f, f"{W}/b.infos"]))
    r = {"file": rel, "bytes": os.path.getsize(f), "bend_ms": stats([x[0] for x in a]), "rust_ms": stats([x[0] for x in b]), "bend_rss_mb": stats([x[1] for x in a]), "rust_rss_mb": stats([x[1] for x in b])}
    res.append(r); print(json.dumps(r), flush=True)
for rel in ("vme-wren/vme-wren.kicad_pcb", "jetson-agx-thor-baseboard/jetson-agx-thor-baseboard.kicad_pcb"):
    f = f"{D}/{rel}"; b = [one([rust, "0", f, f"{W}/b.infos"]) for i in range(n)]
    r = {"file": rel, "bytes": os.path.getsize(f), "bend": "over the 3.7 GiB cap", "rust_ms": stats([x[0] for x in b]), "rust_rss_mb": stats([x[1] for x in b])}
    res.append(r); print(json.dumps(r), flush=True)
json.dump(res, open(out, "w"), indent=0)
