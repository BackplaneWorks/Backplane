"""A project folder as comparable items (the benchmark's "same result"): norm(dir, kicad_cli) -> dict. Ids, text positions and file order are left out;
what an edit means is kept: each footprint's place, side, value and fields; tracks, vias and zones with their fills' area; net classes and their patterns;
custom rules; each symbol's library id, place, value and fields; wires, junctions and labels; and what DRC and ERC say, by kind."""
import collections, glob, json, os, re, subprocess, sys, tempfile

def parse(text):
    tok = re.compile(r'"((?:[^"\\]|\\.)*)"|([()])|([^\s()"]+)')
    stack = [[]]
    for m in tok.finditer(text):
        s, p, a = m.groups()
        if p == "(": stack.append([])
        elif p == ")": x = stack.pop(); stack[-1].append(x)
        else: stack[-1].append(a if s is None else s)
    return stack[0][0]
def kids(n, name): return [c for c in n if isinstance(c, list) and c and c[0] == name]
def kid(n, name): k = kids(n, name); return k[0] if k else None
def val(n, name, i=1, d=""): k = kid(n, name); return k[i] if k and len(k) > i else d
def num(s): return round(float(s), 4)
def at(n): a = kid(n, "at"); return [num(x) for x in a[1:4]] if a else []
def props(n): return {p[1]: p[2] for p in kids(n, "property") if len(p) > 2 and p[2] != ""}
def area(pts):
    s = 0.0
    for i in range(len(pts)): x1, y1 = pts[i]; x2, y2 = pts[(i + 1) % len(pts)]; s += x1 * y2 - x2 * y1
    return abs(s) / 2
def pts(n): p = kid(n, "pts"); return [[num(x[1]), num(x[2])] for x in kids(p, "xy")] if p else []
def bag(xs): return sorted(json.dumps(x, sort_keys=True) for x in xs)

def board(path):
    t = parse(open(path).read()); o = {"footprints": {}, "other": collections.Counter()}
    for f in kids(t, "footprint"):
        p = props(f); a = at(f); tr = kid(f, "transform")
        if tr: a = [num(val(tr, "translate")), num(val(tr, "translate", 2)), num(val(tr, "rotate", d="0"))]  # the newer file format
        o["footprints"][p.get("Reference", "?")] = {"lib": f[1], "at": a[:2], "rot": a[2] if len(a) > 2 else 0, "layer": val(f, "layer"), "props": {k: v for k, v in p.items() if k != "Reference"}, "pads": len(kids(f, "pad"))}
    o["segments"] = bag([sorted([[num(val(s, "start")), num(val(s, "start", 2))], [num(val(s, "end")), num(val(s, "end", 2))]]), num(val(s, "width")), val(s, "layer"), val(s, "net")] for s in kids(t, "segment"))
    o["arcs"] = len(kids(t, "arc"))
    o["vias"] = bag([at(v), num(val(v, "size")), num(val(v, "drill")), kid(v, "layers")[1:], val(v, "net")] for v in kids(t, "via"))
    zs = []
    for z in kids(t, "zone"):
        fill = sum(area(pts(fp)) for fp in kids(z, "filled_polygon"))
        zs.append({"net": val(z, "net"), "name": val(z, "name"), "layers": (kid(z, "layer") or kid(z, "layers") or [])[1:], "priority": val(z, "priority", d="0"),
            "clearance": val(kid(z, "connect_pads") or [], "clearance"), "min_thickness": val(z, "min_thickness"), "outline": [pts(p) for p in kids(z, "polygon")], "fill_mm2": round(fill, 1)})
    o["zones"] = bag(zs)
    for k in ("gr_line", "gr_arc", "gr_poly", "gr_rect", "gr_circle", "gr_text", "group", "dimension"): o["other"][k] = len(kids(t, k))
    o["other"] = dict(o["other"]); return o
def sheet(path):
    t = parse(open(path).read()); o = {"symbols": {}}
    for s in kids(t, "symbol"):
        p = props(s); a = at(s); key = p.get("Reference", "?") + ":" + val(s, "unit", d="1")
        if p.get("Reference", "").startswith("#"): key += "@%s,%s" % tuple(a[:2])
        o["symbols"][key] = {"lib": val(s, "lib_id"), "at": a, "mirror": val(s, "mirror"), "props": {k: v for k, v in p.items() if k != "Reference"}}
    o["wires"] = bag(sorted(pts(w)) for w in kids(t, "wire"))
    o["junctions"] = bag(at(j)[:2] for j in kids(t, "junction"))
    o["labels"] = bag([k, l[1], at(l)[:2]] for k in ("label", "global_label", "hierarchical_label") for l in kids(t, k))
    o["no_connects"] = bag(at(j)[:2] for j in kids(t, "no_connect")); o["sheets"] = len(kids(t, "sheet"))
    return o
def rules(path):
    if not os.path.exists(path): return {}
    t = parse("(" + open(path).read() + ")")
    return {r[1]: bag([c[1:] for c in r[2:] if isinstance(c, list)]) for r in kids(t, "rule")}
def report(cli, args, path):
    with tempfile.TemporaryDirectory() as d:
        out = d + "/r.json"; r = subprocess.run([cli] + args + ["--format", "json", "--severity-all", "-o", out, path], capture_output=True, text=True)
        try: j = json.load(open(out))
        except Exception: return {"failed": (r.stdout + r.stderr)[-300:]}
    c = collections.Counter()
    for v in j.get("violations", []): c[v["type"]] += 1
    for sh in j.get("sheets", []):
        for v in sh.get("violations", []): c[v["type"]] += 1
    c["unconnected_items"] = len(j.get("unconnected_items", []))
    return {k: v for k, v in sorted(c.items()) if v}
def norm(d, cli):
    o = {}
    for p in sorted(glob.glob(d + "/*.kicad_pcb")):
        n = os.path.basename(p); o[n] = board(p); o[n + ":drc"] = report(cli, ["pcb", "drc"], p); o[n[:-10] + ".kicad_dru"] = rules(p[:-10] + ".kicad_dru")
    for p in sorted(glob.glob(d + "/*.kicad_sch")): o[os.path.basename(p)] = sheet(p)
    for p in sorted(glob.glob(d + "/*.kicad_pro")):
        j = json.load(open(p)); ns = j.get("net_settings", {}); o[os.path.basename(p)] = {"classes": {c["name"]: {k: v for k, v in c.items() if k in ("clearance", "track_width", "diff_pair_width", "diff_pair_gap", "via_diameter", "via_drill")} for c in ns.get("classes", [])},
            "patterns": ns.get("netclass_patterns") or [], "assignments": ns.get("netclass_assignments") or {}}
        root = p[:-10] + ".kicad_sch"
        if os.path.exists(root): o[os.path.basename(root) + ":erc"] = report(cli, ["sch", "erc"], root)
    return o
def diff(a, b, path=""):
    """The differences between two norms, as short lines (at most 12)."""
    out = []
    def go(x, y, p):
        if len(out) >= 12: return
        if isinstance(x, dict) and isinstance(y, dict):
            for k in sorted(set(x) | set(y)):
                if k not in x: out.append(f"{p}/{k}: extra {json.dumps(y[k])[:120]}")
                elif k not in y: out.append(f"{p}/{k}: missing (ref {json.dumps(x[k])[:120]})")
                else: go(x[k], y[k], f"{p}/{k}")
        elif isinstance(x, list) and isinstance(y, list) and all(isinstance(i, str) for i in x + y):
            cx, cy = collections.Counter(x), collections.Counter(y)
            for i in (cx - cy): out.append(f"{p}: missing {i[:140]}")
            for i in (cy - cx): out.append(f"{p}: extra {i[:140]}")
        elif x != y: out.append(f"{p}: ref {json.dumps(x)[:100]} got {json.dumps(y)[:100]}")
    go(a, b, path); return out
if __name__ == "__main__":
    a = norm(sys.argv[1], sys.argv[3]); b = norm(sys.argv[2], sys.argv[3]); d = diff(a, b); print("\n".join(d) if d else "same")
