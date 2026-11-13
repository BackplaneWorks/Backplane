#!/usr/bin/env python3
"""The real-use table: one row per measure, one column per board x runtime, each value against its budget.
  real_use_table.py ROOT [--strict]    reads ROOT/<board>-<runtime>/real_use.json; writes ROOT/summary.json; prints markdown
Exits 1 on a miss of a budget marked "strict": true (marked ✗✗), or on any miss with --strict (KEMBED_REAL_STRICT=1, or top-level
"strict": true in real_use_budgets.json)."""
import glob, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CFG = json.load(open(os.path.join(HERE, "..", "real_use_budgets.json")))
BUDGETS = [(b["re"], b["field"], b.get("max"), b["what"], b.get("min"), b.get("strict") is True) for b in CFG["budgets"]]

def main():
    root = sys.argv[1]; strict = "--strict" in sys.argv or os.environ.get("KEMBED_REAL_STRICT") == "1" or CFG.get("strict") is True
    runs = {}
    for f in sorted(glob.glob(os.path.join(root, "*", "real_use.json"))):
        r = runs[os.path.basename(os.path.dirname(f))] = json.load(open(f))
        r["results"]["dialogs"] = {"n": len(r.get("dialogs", [])), "ms": len(r.get("dialogs", []))}
    if not runs: print("no runs"); return 1
    names = []
    for r in runs.values():
        for k in r["results"]:
            if k not in names: names.append(k)
    cols = list(runs)
    print("| measure | " + " | ".join(cols) + " |"); print("|---" * (len(cols) + 1) + "|")
    misses = []
    def cell(run, name):
        v = runs[run]["results"].get(name)
        if not v: return ""
        s = "" if v.get("ms") is None else str(v["ms"])
        if v.get("blank_frames"): s += f" ({v['blank_frames']} blank)"
        if v.get("hitch50"): s += f" ({v['hitch50']} >50)"
        if v.get("lost"): s = "lost"
        if v.get("p50") is not None: s += f" p50 {v['p50']}"
        for k in ("mb", "resizes", "ok"):
            if v.get(k) is not None and v.get("ms") is None: s = f"{k} {v[k]}" + (f" hwm {v['hwm_mb']}" if v.get("hwm_mb") else "")
        marked = False
        for rx, field, lim, what, low, hard in BUDGETS:
            if re.search(rx, name):
                x = v.get(field)
                bad = x is None or (isinstance(x, (int, float)) and x < 0 and low is None) or (lim is not None and x > lim) or (low is not None and x < low)
                if bad:
                    if not marked: s += " ✗✗" if hard else " ✗"; marked = True
                    misses.append((run, name, field, x, lim if low is None else low, what, hard))
        return s
    for n in names: print(f"| {n} | " + " | ".join(cell(c, n) for c in cols) + " |")
    for c in cols:
        for d in runs[c].get("dialogs", []): print(f"dialog {c}: {d['title']}")
    json.dump({"runs": runs, "misses": misses}, open(os.path.join(root, "summary.json"), "w"), indent=1)
    hard = [m for m in misses if m[6]]
    for m in hard: print(f"STRICT MISS {m[0]} {m[1]}: {m[2]}={m[3]} (limit {m[4]}: {m[5]})")
    print(f"budget misses: {len(misses)} (strict: {len(hard)})")
    # a budget marked "strict" fails the run on its own (met once, it must not regress); the rest fail only under --strict
    return 1 if hard or (strict and misses) else 0

if __name__ == "__main__": sys.exit(main())
