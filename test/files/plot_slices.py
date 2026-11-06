#!/usr/bin/env python3
"""plot_slices.py OUT.json ORACLE BPPLOT [MB]: the two demo boards child cannot read whole under the 3.7 GiB cap, cut into valid boards of about MB megabytes
(every top-level form in exactly one slice; `general` copied into each so the thickness is read) and compared slice by slice. Each oracle run gets its own capped scope.
This covers every form's items and chunks; it does not cover one plot of the whole file (its deal, sort and grouping over all pieces at once)."""
import hashlib, json, os, shutil, subprocess, sys
D = os.path.expanduser("~/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos")
out, oracle, rust = sys.argv[1:4]; mb = float(sys.argv[4]) if len(sys.argv) > 4 else 4
W = os.path.abspath("build/gpui/files-run/plotport/slices"); shutil.rmtree(W, ignore_errors=True); os.makedirs(W)
oracle, rust = os.path.abspath(oracle), os.path.abspath(rust)
def forms(b):
    d = 0; s = e = False; st = 0; o = []
    for i, c in enumerate(b):
        if s:
            if e: e = False
            elif c == 92: e = True
            elif c == 34: s = False
        elif c == 34: s = True
        elif c == 40:
            if d == 1: st = i
            d += 1
        elif c == 41:
            if d == 2: o.append((st, i + 1))
            d -= 1
    return o
res = []
for rel in ("vme-wren/vme-wren.kicad_pcb", "jetson-agx-thor-baseboard/jetson-agx-thor-baseboard.kicad_pcb"):
    b = open(f"{D}/{rel}", "rb").read(); fs = forms(b)
    gen = next((b[a:z] for a, z in fs if b[a:a + 9] == b"(general "), b"")
    cur = []; size = 0; parts = []
    for a, z in fs:
        if size and size + z - a > mb * (1 << 20): parts.append(cur); cur = []; size = 0
        cur.append((a, z)); size += z - a
    parts.append(cur)
    for i, p in enumerate(parts):
        f = f"{W}/s.kicad_pcb"
        open(f, "wb").write(b"(kicad_pcb\n" + gen + b"\n" + b"\n".join(b[a:z] for a, z in p) + b"\n)\n")
        one = {"rel": rel, "slice": i, "bytes": os.path.getsize(f), "forms": len(p)}
        for name, w in (("oracle", ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=3700M", "-p", "MemorySwapMax=0", oracle, "--plot", "0", f, f"{W}/o.infos"]), ("rust", [rust, "0", f, f"{W}/r.infos"])):
            r = subprocess.run(w, stdin=subprocess.DEVNULL, capture_output=True)
            one[name] = [r.returncode, hashlib.sha256(r.stdout).hexdigest()]
        same = one["oracle"] == one["rust"] and open(f"{W}/o.infos", "rb").read() == open(f"{W}/r.infos", "rb").read() if one["oracle"][0] == 0 else None
        one["same"] = same; res.append(one)
        print(rel, i, len(parts), one["bytes"], one["oracle"][0], same, flush=True)
        json.dump(res, open(out, "w"), indent=0)
shutil.rmtree(W, ignore_errors=True)
print("slices", len(res), "identical", sum(1 for x in res if x["same"]), "oracle failed", sum(1 for x in res if x["same"] is None), "different", sum(1 for x in res if x["same"] is False))
