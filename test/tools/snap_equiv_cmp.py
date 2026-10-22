#!/usr/bin/env python3
"""Compare a hot boot's dump with the full replay's (S3's equivalence check).

  snap_equiv_cmp.py FULL.jsonl HOT.jsonl [CUT]

The dumps come from test/native/snap_equiv.bend (full / hot). Same rules as
test/snapequiv_lib.bend's Rep.*: records per domain (a table, or
obj.<domain>): equal / missing / extra / differing, the first 3 bugs with
both values; a cold setting (storedb's Sdb.cold) missing from the hot side
is meant. Entries, for each thread the hot boot loaded: the hot list must
start with the newest min(n, WINDOW) of the full list and hold nothing the
full list does not, in order. Exit 1 when anything differs, 3 when the hot
boot refused the store.
"""
import json, sys

WINDOW = 200  # hotboot.bend's Hot.window()
COLD = ("launch.input.run", "fallback.input.run")  # storedb.bend's Sdb.cold

def load(path):
    recs, ents, refused = {}, {}, None
    with open(path, encoding="utf-8") as f:
        for line in f:
            x = json.loads(line)
            if x and x[0] == "REFUSED" and len(x) == 2:
                refused = x[1]
            elif x and x[0] == "E":  # a record's key always holds \x1f, never just "E"
                ents[x[1]] = x[2:]
            else:
                recs[x[0]] = x[1]
    return recs, ents, refused

def dom(k):
    p = k.split("\x1f")
    return "obj." + p[1] if p[0] == "obj" and len(p) > 1 else p[0]

def cold(k):
    p = k.split("\x1f")
    return p[0] == "setting" and len(p) > 1 and p[1].startswith(COLD)

def vis(s, n=240):
    s = s.replace("\x1f", "|").replace("\n", "↵")
    return s if len(s) <= n else s[:n] + "…"

def main():
    full_p, hot_p = sys.argv[1], sys.argv[2]
    cut = sys.argv[3] if len(sys.argv) > 3 else "?"
    hot, hot_e, refused = load(hot_p)
    if refused is not None:
        print(f"cut {cut}: REFUSED Hot.load did not use the store: {refused}")
        return 3
    full, full_e, _ = load(full_p)
    doms = {}
    def c(d):
        return doms.setdefault(d, {"equal": 0, "missing": 0, "cold": 0, "extra": 0, "differing": 0, "ex": []})
    for k, v in full.items():
        d = c(dom(k))
        if k not in hot:
            if cold(k):
                d["cold"] += 1
                continue
            d["missing"] += 1; kind = "missing"
        elif hot[k] == v:
            d["equal"] += 1; continue
        else:
            d["differing"] += 1; kind = "differing"
        if len(d["ex"]) < 3:
            d["ex"].append(f"{kind} {vis(k)}\n        full: {vis(v)}\n        hot:  {vis(hot[k]) if k in hot else '(none)'}")
    for k, v in hot.items():
        if k not in full:
            d = c(dom(k)); d["extra"] += 1
            if len(d["ex"]) < 3:
                d["ex"].append(f"extra {vis(k)}\n        full: (none)\n        hot:  {vis(v)}")
    bad = 0; out = []
    for name in sorted(doms):
        d = doms[name]; b = d["missing"] + d["extra"] + d["differing"]; bad += b
        line = f"  {'DIFF' if b else 'ok  '} {name}: equal {d['equal']}, missing {d['missing']}, extra {d['extra']}, differing {d['differing']}"
        if d["cold"]:
            line += f" (cold settings left out, as meant: {d['cold']})"
        out.append(line)
        out += ["      " + e for e in d["ex"]]
    ebad = 0; eout = []
    for th, h in sorted(hot_e.items()):
        f = full_e.get(th)
        if f is None:
            ebad += 1; eout.append(f"      {th}: no entries dumped for the full replay"); continue
        n = min(len(f), WINDOW); why = ""
        for i in range(n):
            if i >= len(h):
                why = f"hot ends at {i}"; break
            if h[i] != f[i]:
                why = f"at {i}: full {vis(f[i])} | hot {vis(h[i])}"; break
        j = 0
        for x in h:
            while j < len(f) and f[j] != x:
                j += 1
            if j == len(f):
                why += ("; " if why else "") + f"not in full, in order: {vis(x)}"; break
            j += 1
        if why:
            ebad += 1; eout.append(f"      {th} (full {len(f)}, hot {len(h)}): {why}")
    out.append(f"  {len(hot_e)} open threads, {ebad} with a window that differs")
    out += eout
    bad += ebad
    print(f"cut {cut}: " + ("ok" if bad == 0 else f"FAIL {bad} differences"))
    print("\n".join(out))
    return 1 if bad else 0

if __name__ == "__main__":
    sys.exit(main())
