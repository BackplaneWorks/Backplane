#!/usr/bin/env python3
"""latency_summary.py CLIENT.log OUT.json: p50/p95/max of the scripted steps' latency and the mean of each phase, per step kind."""
import json, sys, statistics
rows = {}
for line in open(sys.argv[1]):
    if line.startswith("GPUI_METRIC ") and '"event":"input"' in line:
        m = json.loads(line[12:]); kind = m["step"].split(":")[0]
        rows.setdefault(kind, []).append(m)
def pct(xs, p): xs = sorted(xs); return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))]
out = {}
for kind, ms in rows.items():
    ok = [m for m in ms if m.get("ok")]; lat = [m["latency_ms"] for m in ok]
    d = {"n": len(ms), "ok": len(ok)}
    if lat: d.update({"p50": round(pct(lat, .5), 1), "p95": round(pct(lat, .95), 1), "max": round(max(lat), 1)})
    for ph in ("hub_rtt_ms", "decode_ms", "process_ms", "apply_ms", "render_build_ms", "applied_to_frame_ms", "bytes"):
        v = [m[ph] for m in ok if ph in m]
        if v: d["mean_" + ph] = round(statistics.mean(v), 2)
    d["delta_fraction"] = round(sum(1 for m in ok if m.get("delta")) / max(1, len(ok)), 2)
    out[kind] = d
json.dump(out, open(sys.argv[2], "w"), indent=1)
for k, d in out.items(): print(k, json.dumps(d))
