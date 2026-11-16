#!/usr/bin/env python3
"""score_all.py H : quality, a picture and the trace summary of every head-to-head run under H/{a,b,c}; one JSON per run in H/score."""
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import h2h
H = sys.argv[1]; cli = json.load(open(H + "/a/bench.json"))["cli"]; os.makedirs(H + "/score", exist_ok=True)
for v in "abc":
    for l in open(f"{H}/{v}/results.jsonl"):
        r = json.loads(l); name = f"{r['variant']}-{r['task']}-{r['run']}"; out = f"{H}/score/{name}.json"
        if os.path.exists(out): continue
        run = f"{H}/{v}/runs/{name}"
        try: q = h2h.quality(run, r["task"], H + "/fx", cli)
        except Exception as e: q = {"error": str(e)[:200]}
        try: h2h.render(run, r["task"], cli, f"{H}/score/{name}.png", H + "/fx")
        except Exception as e: q["render_error"] = str(e)[:200]
        t = h2h.trace(f"{H}/{v}/out/{name}.jsonl")
        json.dump({"run": {k: r.get(k) for k in ("variant", "task", "run", "pass", "context", "output", "calls", "wall_s", "cost", "diff", "tools")}, "quality": q, "trace": t}, open(out, "w"), indent=1)
        print(name, "pass" if r["pass"] else "FAIL", {k: q.get(k) for k in ("ratio", "vias", "layers", "bends", "acute", "margin_over_rule_mm", "drc_on_nets", "unconnected_on_nets")}, "| routerish", t["routerish_calls"], "reads", t["board_reads"], "failed", t["failed_calls"], t["tools"], flush=True)
