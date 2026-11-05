#!/usr/bin/env python3
"""mesh_compare.py OUT.json ORACLE BPPLOT DIR... : every .glb in the DIRs through the Bend child and through Rust, compared byte for byte.
 solid: `ORACLE --plot K FILE INFOS` against `BPPLOT K FILE INFOS`, K 2 and 3 (exit code, stdout, stderr, info file);
 render: `ORACLE --render FILE` against `BPPLOT --render FILE`, in folders of their own: stdout, exit, and each PNG's bytes; a PNG that differs is decoded and its differing pixels counted.
Every child runs under MemoryMax=3700M, MemorySwapMax=0. HOME and XDG_CACHE_HOME are temp folders."""
import hashlib, json, os, shutil, struct, subprocess, sys, tempfile, time, zlib
out, oracle, rust = sys.argv[1:4]; dirs = sys.argv[4:]
W = tempfile.mkdtemp(prefix="meshcmp-", dir=os.path.abspath("build/gpui/files-run"))
ENV = {**os.environ, "HOME": W, "XDG_CACHE_HOME": W + "/cache", "BACKPLANE_NO_UPDATE": "1"}
def run(argv, cwd="/"):
    t = time.time()
    p = subprocess.run(["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=3700M", "-p", "MemorySwapMax=0", "--"] + argv, cwd=cwd, env=ENV, stdin=subprocess.DEVNULL, capture_output=True)
    return p.returncode, p.stdout, p.stderr, time.time() - t
def rows(png):
    assert png[:8] == b"\x89PNG\r\n\x1a\n"; at = 8; idat = b""; w = h = 0
    while at < len(png):
        n, ty = struct.unpack(">I4s", png[at:at + 8]); d = png[at + 8:at + 8 + n]; at += 12 + n
        if ty == b"IHDR": w, h = struct.unpack(">II", d[:8])
        if ty == b"IDAT": idat += d
    raw = zlib.decompress(idat); px = bytearray(); st = w * 3
    for y in range(h):
        r = bytearray(raw[y * (st + 1) + 1:(y + 1) * (st + 1)]); assert raw[y * (st + 1)] == 1
        for i in range(3, st): r[i] = (r[i] + r[i - 3]) & 255
        px += r
    return w, h, bytes(px)
def diff(a, b):
    (w, h, p), (_, _, q) = rows(a), rows(b)
    bad = [i for i in range(w * h) if p[i * 3:i * 3 + 3] != q[i * 3:i * 3 + 3]]
    return {"pixels": len(bad), "first": [[i % w, i // w, p[i * 3:i * 3 + 3].hex(), q[i * 3:i * 3 + 3].hex()] for i in bad[:5]], "most": max((max(abs(p[i * 3 + k] - q[i * 3 + k]) for k in range(3)) for i in bad), default=0)}
# what a render says: stdout and stderr together, as the hub reads them, less the folder it ran in and the Bend runtime's own stall reports (`[bp-loop] {...}`, timings, not output)
def said(r, d): return b"\n".join(l for l in (r[1] + r[2]).replace(d.encode(), b"").split(b"\n") if l.strip() and not l.startswith(b"[bp-loop]"))
res = {"solid": [], "render": []}
files = sorted(os.path.abspath(os.path.join(d, f)) for d in dirs for f in os.listdir(d) if f.endswith(".glb"))
same_s = same_r = pngs = pngs_same = 0
for f in files:
    name = os.path.basename(f)
    for k in (2, 3):
        ia, ib = f"{W}/a.info", f"{W}/b.info"
        for i in (ia, ib):
            if os.path.exists(i): os.remove(i)
        a, b = run([oracle, "--plot", str(k), f, ia]), run([rust, str(k), f, ib])
        info = lambda p: open(p, "rb").read() if os.path.exists(p) else None
        same = a[0] == b[0] and a[1] == b[1] and info(ia) == info(ib) and (a[0] == 0 or a[2].strip() == b[2].strip())
        same_s += same
        res["solid"].append({"file": name, "kind": k, "same": same, "exit": [a[0], b[0]], "mesh": b"\nm" in a[1], "bytes": [len(a[1]), len(b[1])], "sha": hashlib.sha256(b[1]).hexdigest()[:16], "bend_s": round(a[3], 3), "rust_s": round(b[3], 3)} | ({} if same else {"bend_err": a[2][-300:].decode("latin1"), "rust_err": b[2][-300:].decode("latin1")}))
    da, db = f"{W}/ra", f"{W}/rb"
    for d in (da, db):
        shutil.rmtree(d, ignore_errors=True); os.makedirs(d); shutil.copy(f, d + "/m.glb")
    a, b = run([oracle, "--render", "m.glb"], da), run([rust, "--render", "m.glb"], db)
    r = {"file": name, "exit": [a[0], b[0]], "bend_s": round(a[3], 3), "rust_s": round(b[3], 3), "views": []}
    pa = sorted(os.listdir(da + "/renders")) if os.path.isdir(da + "/renders") else []
    pb = sorted(os.listdir(db + "/renders")) if os.path.isdir(db + "/renders") else []
    ok = a[0] == b[0] and pa == pb and said(a, da) == said(b, db)
    if not ok: r["stdout"] = [a[1][-300:].decode("latin1"), b[1][-300:].decode("latin1")]; r["stderr"] = [a[2][-300:].decode("latin1"), b[2][-300:].decode("latin1")]
    for v in sorted(set(pa) & set(pb)):
        x, y = open(f"{da}/renders/{v}", "rb").read(), open(f"{db}/renders/{v}", "rb").read()
        pngs += 1; e = x == y; pngs_same += e; ok &= e
        r["views"].append({"view": v, "same": e, "bytes": len(y), "sha": hashlib.sha256(y).hexdigest()[:16]} | ({} if e else diff(x, y)))
    r["same"] = ok; same_r += ok; res["render"].append(r)
    print(name, "solid", [x["same"] for x in res["solid"][-2:]], "render", ok, r["exit"], f"{r['bend_s']}s/{r['rust_s']}s", flush=True)
res["summary"] = {"files": len(files), "solid_same": same_s, "solid_cases": len(res["solid"]), "solid_with_mesh": sum(x["mesh"] for x in res["solid"]), "render_same": same_r, "pngs": pngs, "pngs_identical": pngs_same}
print(res["summary"]); json.dump(res, open(out, "w"), indent=0); shutil.rmtree(W, ignore_errors=True)
sys.exit(0 if same_s == len(res["solid"]) and same_r == len(files) else 1)
