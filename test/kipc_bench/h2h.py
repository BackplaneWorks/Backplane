#!/usr/bin/env python3
"""The routing head-to-head's scoring (docs/kicad-ipc-bench.md): what a run's routes are like, a picture of them, and what its agent did.
  h2h.py quality RUN_DIR TASK FIXTURES CLI     -> JSON: per net length against the shortest tree over its pads, vias, layers, bends, acute
                                                  angles, DRC findings and the clearance the new copper keeps over the rule
  h2h.py render RUN_DIR TASK CLI OUT.png       -> the routed area's copper layers as a PNG (kicad-cli's SVG, cut to the nets' pads)
  h2h.py trace LOG.jsonl                       -> JSON: the tool calls in order (short), the largest requests, what looks like a home-made
                                                  router, re-reads of the board, failed calls"""
import json, math, os, re, shutil, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import kx, norm

TASKS = {"route_leds": ("StickHub", [f"/LED{i}" for i in range(1, 8)], 0.15, ["B.Cu", "F.Cu"]),
         "route_drv": ("StickHub", [f"Net-(D{i}-1)" for i in range(15, 22)], 0.15, ["B.Cu", "F.Cu"]),
         "hubble_route": ("Hubble_DK", ["Net-(U9-B1)", "Net-(U9-B2)", "Net-(U9-B3)", "/Debugger/DBG_LS_OE"], 0.1, ["F.Cu", "In2.Cu", "In3.Cu", "In5.Cu", "In6.Cu", "B.Cu"])}

def copper(pcb, nets):
    t = norm.parse(open(pcb).read()); segs, vias = [], []
    for k in ("segment", "arc"):
        for s in norm.kids(t, k):
            n = norm.val(s, "net")
            if n in nets: a, b = norm.kid(s, "start"), norm.kid(s, "end"); segs.append((n, (float(a[1]), float(a[2])), (float(b[1]), float(b[2])), float(norm.val(s, "width") or 0), norm.val(s, "layer"), k))
    for v in norm.kids(t, "via"):
        n = norm.val(v, "net")
        if n in nets: a = norm.kid(v, "at"); vias.append((n, (float(a[1]), float(a[2])), float(norm.val(v, "size") or 0), [x for x in (norm.kid(v, "layers") or [])[1:]], "micro" if "micro" in v[:3] else "blind" if "blind" in v[:3] else "through"))
    padnet = {}
    for f in norm.kids(t, "footprint"):
        for p in norm.kids(f, "pad"):
            n = norm.kid(p, "net")
            if n and n[-1] in nets: padnet[(norm.props(f).get("Reference"), p[1])] = n[-1]
    return segs, vias, padnet
def mst(pts):
    if len(pts) < 2: return 0.0
    inn = {0}; tot = 0.0
    while len(inn) < len(pts):
        d, j = min((math.dist(pts[i], pts[k]), k) for i in inn for k in range(len(pts)) if k not in inn); tot += d; inn.add(j)
    return tot
def bends(segs):
    """bends and acute corners where two tracks of a net meet end to end on a layer (a corner under 90 degrees inside is acute)"""
    b = a = 0; ends = {}
    for i, (n, p, q, w, l, k) in enumerate(segs):
        for e, o in ((p, q), (q, p)): ends.setdefault((n, l, round(e[0], 4), round(e[1], 4)), []).append((i, o, e))
    for key, v in ends.items():
        if len(v) != 2: continue
        (i, o1, e), (j, o2, _) = v; u = (o1[0] - e[0], o1[1] - e[1]); w = (o2[0] - e[0], o2[1] - e[1]); lu, lw = math.hypot(*u), math.hypot(*w)
        if lu < 1e-6 or lw < 1e-6: continue
        ang = math.degrees(math.acos(max(-1, min(1, (u[0] * w[0] + u[1] * w[1]) / (lu * lw)))))  # 180 = straight on
        if ang < 179: b += 1
        if ang < 89: a += 1
    return b, a
def margin(run, name, nets, rule, cli):
    """the largest extra clearance (over the rule) the nets' copper keeps to other nets' pads, tracks and vias: KiCad's own DRC asked with a
    custom rule at rule + m (pours left out: a pour is filled to exactly the rule)"""
    steps = [0.0, 0.01, 0.025, 0.05, 0.1, 0.15, 0.2]; ok = None
    for m in steps:
        with tempfile.TemporaryDirectory() as tmp:
            d = tmp + "/p"; shutil.copytree(run, d, ignore=shutil.ignore_patterns("history.git", ".backplane", "*.lck")); dru = f"{d}/{name}.kicad_dru"
            old = open(dru).read() if os.path.exists(dru) else "(version 1)\n"
            cond = " || ".join(f"A.NetName == '{n}'" for n in nets)
            open(dru, "w").write(old + f'\n(rule "h2h_margin"\n\t(condition "({cond}) && A.Type != \'Zone\' && B.Type != \'Zone\'")\n\t(constraint clearance (min {rule + m:.4f}mm)))\n')
            r = kx.drc(cli, f"{d}/{name}.kicad_pcb")
            hit = [v for v in r["violations"] if v["type"] == "clearance" and "h2h_margin" in v.get("description", "")]
        if hit: break
        ok = m
    return ok
def quality(run, task, fx, cli):
    name, nets, rule, _ = TASKS[task]; pcb = f"{run}/{name}.kicad_pcb"; fixt = [k for k in os.listdir(fx) if os.path.exists(f"{fx}/{k}/{name}.kicad_pcb") and (task != "route_leds" or k == "sh_route") and (task != "route_drv" or k == "sh_drv")][0]
    segs, vias, padnet = copper(pcb, set(nets)); s0, v0, _ = copper(f"{fx}/{fixt}/{name}.kicad_pcb", set(nets)); P = kx.pads(cli, pcb)
    old = {(n, p, q, l) for n, p, q, w, l, k in s0}; new = [s for s in segs if (s[0], s[1], s[2], s[4]) not in old]; newv = [v for v in vias if (v[0], v[1]) not in {(x[0], x[1]) for x in v0}]
    per = {}
    for n in nets:
        pts = [P[k] for k, nn in padnet.items() if nn == n and k in P]; L = sum(math.dist(p, q) for nn, p, q, *_ in segs if nn == n); lb = mst(pts)
        per[n] = {"length": round(L, 2), "shortest_tree": round(lb, 2), "ratio": round(L / lb, 2) if lb else None, "vias": sum(1 for v in newv if v[0] == n), "layers": sorted({s[4] for s in new if s[0] == n}), "tracks": sum(1 for s in new if s[0] == n), "widths": sorted({s[3] for s in new if s[0] == n})}
    b, a = bends(new); r = kx.drc(cli, pcb)
    on = lambda v: any(kx.nettag(i["description"]) in nets for i in v["items"])
    L, lb = sum(x["length"] for x in per.values()), sum(x["shortest_tree"] for x in per.values())
    return {"task": task, "length": round(L, 2), "shortest_tree": round(lb, 2), "ratio": round(L / lb, 3) if lb else None, "vias": len(newv), "via_kinds": sorted({v[4] for v in newv}), "layers": sorted({s[4] for s in new}), "tracks": len(new), "bends": b, "acute": a,
            "drc_on_nets": sorted({v["type"] for v in r["violations"] if on(v)}), "drc_violations": len(r["violations"]), "unconnected_on_nets": sum(1 for u in r["unconnected_items"] if on(u)),
            "margin_over_rule_mm": margin(run, name, nets, rule, cli), "nets": per}
def render(run, task, cli, out, fx=None):
    import cairosvg
    name, nets, _, layers = TASKS[task]; pcb = f"{run}/{name}.kicad_pcb"; _, _, padnet = copper(pcb, set(nets)); P = kx.pads(cli, pcb); pts = [P[k] for k in padnet if k in P]
    segs, vias, _ = copper(pcb, set(nets)); pts += [p for s in segs for p in (s[1], s[2])]
    # the task's own new copper is drawn over the dimmed board: yellow on the top layer, cyan on the bottom, magenta and green inside, vias white
    old_s, old_v = set(), set()
    if fx:
        fixt = [k for k in os.listdir(fx) if os.path.exists(f"{fx}/{k}/{name}.kicad_pcb") and (task != "route_leds" or k == "sh_route") and (task != "route_drv" or k == "sh_drv")][0]
        s0, v0, _ = copper(f"{fx}/{fixt}/{name}.kicad_pcb", set(nets)); old_s = {(n, p, q, l) for n, p, q, w, l, k in s0}; old_v = {(v[0], v[1]) for v in v0}
    col = lambda l: {"F.Cu": "#ffd400", "B.Cu": "#00e5ff"}.get(l, "#ff4dff" if l in ("In5.Cu", "In2.Cu") else "#7dff5a")
    newpts = [p for n, a, b, w, l, k in segs if (n, a, b, l) not in old_s for p in (a, b)] + [v[1] for v in vias if (v[0], v[1]) not in old_v]
    if fx and newpts: pts = newpts  # the picture is of what was routed (a net's far pads would shrink it to nothing)
    over = "".join(f'<line x1="{p[0]}" y1="{p[1]}" x2="{q[0]}" y2="{q[1]}" stroke="{col(l)}" stroke-width="{max(w, 0.08)}" stroke-linecap="round"/>' for n, p, q, w, l, k in segs if (n, p, q, l) not in old_s)
    over += "".join(f'<circle cx="{v[1][0]}" cy="{v[1][1]}" r="{v[2] / 2}" fill="#ffffff" stroke="#000" stroke-width="0.03"/>' for v in vias if (v[0], v[1]) not in old_v)
    x0, y0, x1, y1 = min(p[0] for p in pts) - 1.5, min(p[1] for p in pts) - 1.5, max(p[0] for p in pts) + 1.5, max(p[1] for p in pts) + 1.5
    with tempfile.TemporaryDirectory() as tmp:
        # the pours' fills are left out of the picture (they hide the tracks): the board copied without its filled polygons
        t = open(pcb).read(); out_t = []; i = 0
        while True:
            k = t.find("(filled_polygon", i)
            if k < 0: out_t.append(t[i:]); break
            out_t.append(t[i:k]); d = 0; j = k
            while True:
                if t[j] == "(": d += 1
                elif t[j] == ")":
                    d -= 1
                    if d == 0: break
                j += 1
            i = j + 1
        shutil.copytree(run, tmp + "/p", ignore=shutil.ignore_patterns("history.git", ".backplane", "*.lck")); pcb = f"{tmp}/p/{name}.kicad_pcb"; open(pcb, "w").write("".join(out_t))
        kx.run(cli, "pcb", "export", "svg", "--layers", ",".join(layers + ["Edge.Cuts"]), "--mode-single", "--exclude-drawing-sheet", "--page-size-mode", "0", "-o", tmp + "/b.svg", pcb)
        s = open(tmp + "/b.svg").read()
        s = s.replace("</svg>", f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" fill="#101418" opacity="0.6"/>{over}</svg>')
        s = re.sub(r'viewBox="[^"]*"', f'viewBox="{x0:.3f} {y0:.3f} {x1 - x0:.3f} {y1 - y0:.3f}"', s, count=1); w = 1100; h = int(w * (y1 - y0) / (x1 - x0))
        s = re.sub(r'width="[^"]*"', f'width="{w}"', s, count=1); s = re.sub(r'height="[^"]*"', f'height="{h}"', s, count=1)
        cairosvg.svg2png(bytestring=s.encode(), write_to=out, background_color="#101418")
    return out
ROUTERISH = re.compile(r"heapq|a\*|astar|A\*|dijkstra|shapely|def route|clearance|LineString|bfs|grid\[|visited", re.I)
def trace(log):
    calls, reqs, cur = [], [], None; results = {}
    for l in open(log):
        try: _, j = l.split("\t", 1); j = json.loads(j)
        except ValueError: continue
        if j.get("type") == "assistant":
            u = j["message"].get("usage", {}); ctx = u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
            for b in j["message"].get("content", []):
                if b.get("type") == "tool_use":
                    i = b["input"]; name = b["name"].replace("mcp__backplane__", ""); body = i.get("command") or json.dumps(i.get("ops") or i.get("message") or i)
                    if name == "kicad_ipc": name += ":" + str(i.get("type", "")).rsplit(".", 1)[-1]
                    calls.append({"id": b["id"], "tool": name, "ctx": ctx, "out": u.get("output_tokens", 0), "chars": len(body), "head": body[:160], "routerish": bool(ROUTERISH.search(body)) and name in ("Bash", "Write"), "reads_board": bool(re.search(r"\.kicad_pcb", body)) and name in ("Bash", "Read")})
        if j.get("type") == "user":
            c = j["message"].get("content", [])
            for b in c if isinstance(c, list) else []:
                if b.get("type") == "tool_result":
                    x = json.dumps(b.get("content", "")); results[b.get("tool_use_id")] = (len(x), bool(b.get("is_error")) or '"error"' in x[:40] or "AS_BAD_REQUEST" in x[:300] or "Exit code" in x[:30], x[:160])
    for c in calls: c["result_chars"], c["failed"], c["result_head"] = results.get(c.pop("id"), (0, False, ""))
    tools = {}
    for c in calls: tools[c["tool"]] = tools.get(c["tool"], 0) + 1
    big = sorted(calls, key=lambda c: -c["result_chars"])[:4]
    return {"calls": len(calls), "tools": tools, "routerish_calls": sum(c["routerish"] for c in calls), "board_reads": sum(c["reads_board"] for c in calls), "failed_calls": sum(c["failed"] for c in calls),
            "peak_ctx": max([c["ctx"] for c in calls] or [0]), "largest_results": [(c["tool"], c["result_chars"], c["head"][:80]) for c in big], "seq": calls}
if __name__ == "__main__":
    c = sys.argv[1]
    if c == "quality": print(json.dumps(quality(*sys.argv[2:6]), indent=1))
    elif c == "render": print(render(*sys.argv[2:7]))
    elif c == "trace": t = trace(sys.argv[2]); t.pop("seq"); print(json.dumps(t, indent=1))
