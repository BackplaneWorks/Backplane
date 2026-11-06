#!/usr/bin/env python3
"""Runs a plot reader over every .kicad_pcb/.kicad_sch of the KiCad demos, one process per file, and records for each:
exit code, wall ms, peak RSS (wait4), sha256 of stdout and of the info file. Outputs stay in OUT/<n>.out and OUT/<n>.infos.

  plot_corpus.py OUT.json OUTDIR 'CMD' [--only SUBSTR] [--samples N] [--keep]

CMD holds the reader's words before KIND FILE INFOS, e.g. "build/gpui/backplane-serve --plot" or "build/gpui/files-bin/bp-plot".
Run it inside `systemd-run --user --scope -p MemoryMax=3700M -p MemorySwapMax=0`: nothing here raises a limit of its own.
A file the reader cannot do within the cap is recorded with its exit code, never dropped."""
import hashlib, json, os, shlex, subprocess, sys, time
D = os.path.expanduser("~/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos")
out, odir, cmd = sys.argv[1:4]
a = sys.argv[4:]
only = a[a.index("--only") + 1] if "--only" in a else ""
samples = int(a[a.index("--samples") + 1]) if "--samples" in a else 1
keep = "--keep" in a
os.makedirs(odir, exist_ok=True)
fs = []
for r, _, ns in os.walk(D):
    for n in ns:
        if n.endswith((".kicad_pcb", ".kicad_sch")): fs.append(os.path.join(r, n))
fs.sort(key=lambda p: (os.path.getsize(p), p))
def sha(p):
    h = hashlib.sha256()
    try:
        with open(p, "rb") as f:
            for b in iter(lambda: f.read(1 << 20), b""): h.update(b)
    except OSError: return ""
    return h.hexdigest()
def one(words, so):
    t0 = time.perf_counter()
    with open(so, "wb") as o: p = subprocess.Popen(words, stdin=subprocess.DEVNULL, stdout=o, stderr=subprocess.PIPE)
    err = b""
    _, st, ru = os.wait4(p.pid, 0)
    t = (time.perf_counter() - t0) * 1000
    err = p.stderr.read(400); p.stderr.close()
    return os.waitstatus_to_exitcode(st), t, ru.ru_maxrss, err.decode("utf-8", "replace")
res = []
for f in fs:
    rel = os.path.relpath(f, D)
    if only and only not in rel: continue
    kind = 0 if f.endswith(".kicad_pcb") else 1
    tag = hashlib.sha256(rel.encode()).hexdigest()[:12]
    so, si = f"{odir}/{tag}.out", f"{odir}/{tag}.infos"
    ms, rss = [], []
    for i in range(samples):
        if os.path.exists(si): os.remove(si)
        code, t, m, err = one(shlex.split(cmd) + [str(kind), f, os.path.abspath(si)], so)
        ms.append(round(t, 2)); rss.append(m)
    res.append({"rel": rel, "tag": tag, "kind": kind, "size": os.path.getsize(f), "code": code, "ms": ms, "rss_kb": rss, "out": sha(so), "infos": sha(si), "err": err})
    print(rel, code, ms[-1], "ms", rss[-1] // 1024, "MB", flush=True)
    if not keep and os.path.getsize(so) > (32 << 20):
        os.remove(so)
        if os.path.exists(si): os.remove(si)
    json.dump(res, open(out, "w"), indent=0)
