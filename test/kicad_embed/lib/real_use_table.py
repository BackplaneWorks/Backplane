#!/usr/bin/env python3
"""The real-use table: one row per measure, one column per board x runtime, each value against its budget.
  real_use_table.py ROOT [--strict]    reads ROOT/<board>-<runtime>/real_use.json; writes ROOT/summary.json; prints markdown
--strict (KEMBED_REAL_STRICT=1, or "strict": true in real_use_budgets.json) exits 1 when a budgeted value misses."""
import glob, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CFG = json.load(open(os.path.join(HERE, "..", "real_use_budgets.json")))
BUDGETS = [(b["re"], b["field"], b.get("max"), b["what"], b.get("min")) for b in CFG["budgets"]]

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
        for rx, field, lim, what, low in BUDGETS:
            if re.search(rx, name):
                x = v.get(field)
                bad = x is None or (isinstance(x, (int, float)) and x < 0 and low is None) or (lim is not None and x > lim) or (low is not None and x < low)
                if bad: s += " ✗"; misses.append((run, name, field, x, lim if low is None else low, what)); break
        return s
    for n in names: print(f"| {n} | " + " | ".join(cell(c, n) for c in cols) + " |")
    for c in cols:
        for d in runs[c].get("dialogs", []): print(f"dialog {c}: {d['title']}")
    json.dump({"runs": runs, "misses": misses}, open(os.path.join(root, "summary.json"), "w"), indent=1)
    print(f"budget misses: {len(misses)}")
    return 1 if strict and misses else 0

if __name__ == "__main__": sys.exit(main())
