#!/usr/bin/env python3
"""mesh_bench.py OUT.json ORACLE BPPLOT GLBDIR [N]: wall time (spawn to exit) and peak RSS (wait4) of child and of Rust on demo-board GLBs, N samples each, interleaved:
`--plot 2 FILE INFOS` (the 3D watch's read of a model, no export) and `--render FILE` (all four views). A cold export by kicad-cli is the same process in both and is left out."""
import json, os, shutil, statistics as st, subprocess, sys, time
out, oracle, rust, d = sys.argv[1:5]; n = int(sys.argv[5]) if len(sys.argv) > 5 else 20
W = os.path.abspath("build/gpui/files-run/mesh/bench"); shutil.rmtree(W, ignore_errors=True); os.makedirs(W)
ENV = {**os.environ, "HOME": W, "XDG_CACHE_HOME": W + "/c"}
def one(w, cwd):
    t = time.perf_counter()
    with open(f"{W}/o", "wb") as o: p = subprocess.Popen(w, stdin=subprocess.DEVNULL, stdout=o, stderr=subprocess.DEVNULL, cwd=cwd, env=ENV)
    _, s, ru = os.wait4(p.pid, 0)
    assert os.waitstatus_to_exitcode(s) == 0, w
    return (time.perf_counter() - t) * 1000, ru.ru_maxrss / 1024
def stats(v): v = sorted(v); return {"median": round(st.median(v), 1), "p95": round(v[min(len(v) - 1, int(0.95 * len(v)))], 1), "n": len(v)}
res = {"cores": os.cpu_count(), "load_start": os.getloadavg(), "rows": []}
for name, plot in (("ecc83-pp-parts", True), ("StickHub-parts", True), ("CM5_MINIMA_3-parts", True), ("pic_programmer-parts", True), ("video-parts", False)):
    f = os.path.abspath(f"{d}/{name}.glb"); m = f"{W}/m.glb"; shutil.copy(f, m)
    for what, a, b in ((("plot"), [oracle, "--plot", "2", m, f"{W}/a.i"], [rust, "2", m, f"{W}/b.i"]), ("render", [oracle, "--render", "m.glb"], [rust, "--render", "m.glb"])):
        if what == "plot" and not plot: continue
        k = n if os.path.getsize(f) < 20 << 20 else max(5, n // 4)
        x, y = [], []
        for i in range(k): x.append(one(a, W)); y.append(one(b, W))
        r = {"file": name, "what": what, "bytes": os.path.getsize(f), "ms": stats([v[0] for v in x]), "rust_ms": stats([v[0] for v in y]), "rss_mb": stats([v[1] for v in x]), "rust_rss_mb": stats([v[1] for v in y])}
        res["rows"].append(r); print(json.dumps(r), flush=True)
res["load_end"] = os.getloadavg(); json.dump(res, open(out, "w"), indent=0)
