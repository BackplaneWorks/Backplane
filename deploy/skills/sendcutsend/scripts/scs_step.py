# The STEP probe of scs.py: run inside FreeCAD's console (freecadcmd -c "exec(open('scs_step.py').read())"),
# with SCS_STEP_IN (the STEP) and SCS_STEP_OUT (where the JSON goes). Everything it writes is in inches.
#   thickness   the most common distance between opposite plate faces
#   bends       coaxial cylinder pairs (r, r + t): inside radius, angle, length, the straight flanges beside them,
#               and the closest hole edge to the bend's start (3D)
#   holes       full cylinders through the sheet: diameter, edge distance (to the outline's faces)
#   flat_loops  for a part with no bends: the outline and holes of its largest plate face, in its plane
#   flat_size   for a part whose bends are all parallel: estimated from volume / thickness / width
import json, math, os, traceback

IN = 25.4


def run():
    import FreeCAD as App
    import Part
    shape = Part.Shape()
    shape.read(os.environ["SCS_STEP_IN"])
    solids = shape.Solids
    if not solids:
        return {"error": "no solid in the STEP"}
    out = {"solids": len(solids)}
    if len(solids) > 1:
        out["note"] = f"{len(solids)} solids: SendCutSend quotes one part per file; measured the largest"
    s = max(solids, key=lambda x: x.Volume)
    faces = s.Faces
    kind = lambda f: type(f.Surface).__name__
    planes = [f for f in faces if kind(f) == "Plane"]
    cyls = [f for f in faces if kind(f) == "Cylinder"]

    def normal(f):
        u0, u1, v0, v1 = f.ParameterRange
        return f.normalAt((u0 + u1) / 2, (v0 + v1) / 2)

    # thickness
    weight = {}
    for i, a in enumerate(planes):
        na = normal(a)
        for b in planes[i + 1:]:
            nb = normal(b)
            if na.dot(nb) < -0.9999:
                d = abs((b.CenterOfMass - a.CenterOfMass).dot(na))
                if d > 1e-3:
                    k = round(d, 3)
                    weight[k] = weight.get(k, 0) + min(a.Area, b.Area)
    if not weight:
        return {"error": "no opposite plate faces: this does not look like a sheet part"}
    t = max(weight, key=weight.get)
    out["thickness"] = t / IN

    plate = []
    for a in planes:
        na = normal(a)
        for b in planes:
            if b is a:
                continue
            if na.dot(normal(b)) < -0.9999 and abs(abs((b.CenterOfMass - a.CenterOfMass).dot(na)) - t) < max(0.01, 0.02 * t):
                plate.append(a)
                break

    # cylinders, grouped by axis line and radius
    groups = []
    for f in cyls:
        c = f.Surface
        ax = c.Axis.normalize()
        u0, u1, v0, v1 = f.ParameterRange
        for g in groups:
            gax = g["axis"]
            off = (c.Center - g["center"])
            if abs(abs(ax.dot(gax)) - 1) < 1e-6 and (off - gax * off.dot(gax)).Length < 1e-3 and abs(c.Radius - g["r"]) < 1e-3:
                g["faces"].append(f)
                g["sweep"] += math.degrees(u1 - u0)
                g["height"] = max(g["height"], v1 - v0)
                break
        else:
            groups.append({"axis": ax, "center": c.Center, "r": c.Radius, "faces": [f], "sweep": math.degrees(u1 - u0), "height": v1 - v0})
    holes = [g for g in groups if g["sweep"] > 359 and abs(g["height"] - t) < max(0.02, 0.05 * t)]
    bends, used = [], set()
    for i, g in enumerate(groups):
        if g in holes or i in used:
            continue
        for j, h in enumerate(groups):
            if j == i or j in used or h in holes:
                continue
            off = h["center"] - g["center"]
            same_axis = abs(abs(g["axis"].dot(h["axis"])) - 1) < 1e-6 and (off - g["axis"] * off.dot(g["axis"])).Length < 1e-3
            if same_axis and abs(abs(h["r"] - g["r"]) - t) < max(0.01, 0.02 * t):
                inner, outer = (g, h) if g["r"] < h["r"] else (h, g)
                bends.append({"inner": inner, "outer": outer})
                used.update((i, j))
                break
    bend_faces = {id(f) for b in bends for f in b["inner"]["faces"] + b["outer"]["faces"]}
    hole_faces = {id(f) for g in holes for f in g["faces"]}
    plate_ids = {id(f) for f in plate}
    rim = [f for f in faces if id(f) not in bend_faces and id(f) not in hole_faces and id(f) not in plate_ids]

    hs = []
    for g in holes:
        comp = Part.makeCompound(g["faces"])
        e = min((comp.distToShape(r)[0] for r in rim), default=None)
        hs.append({"d": 2 * g["r"] / IN, "edge": None if e is None else e / IN, "_c": comp})
    webs = []
    for i in range(len(hs)):
        for j in range(i + 1, len(hs)):
            webs.append(hs[i]["_c"].distToShape(hs[j]["_c"])[0] / IN)
    out["holes"] = [{k: v for k, v in h.items() if not k.startswith("_")} for h in hs]
    out["hole_webs"] = webs

    bs = []
    for b in bends:
        inner = b["inner"]
        sweep = inner["sweep"]
        comp = Part.makeCompound(inner["faces"] + b["outer"]["faces"])
        flanges = []
        for f in inner["faces"]:
            for e in f.Edges:
                if type(e.Curve).__name__ not in ("Line", "LineSegment"):
                    continue
                p0, p1 = e.Vertexes[0].Point, e.Vertexes[-1].Point
                d = (p1 - p0)
                if d.Length < 1e-6:
                    continue
                d.normalize()
                for pf in plate:
                    if any(e.isSame(x) for x in pf.Edges):
                        ext = max(((v.Point - p0) - d * (v.Point - p0).dot(d)).Length for v in pf.Vertexes)
                        flanges.append(ext / IN)
        hd = min((h["_c"].distToShape(comp)[0] for h in hs), default=None)
        bs.append({"radius": inner["r"] / IN, "angle": sweep, "length": inner["height"] / IN, "flanges": flanges, "hole_dist": None if hd is None else hd / IN})
    out["bends"] = bs

    rim_len = (sum(f.Area for f in rim) + sum(f.Area for g in holes for f in g["faces"])) / t
    out["cut_length"] = rim_len / IN

    if not bends:
        big = max(planes, key=lambda f: f.Area)
        n = normal(big)
        o = big.CenterOfMass
        ref = App.Vector(1, 0, 0) if abs(n.x) < 0.9 else App.Vector(0, 1, 0)
        u = n.cross(ref).normalize()
        v = n.cross(u)
        loops = []
        for w in big.Wires:
            pts = [((p - o).dot(u) / IN, (p - o).dot(v) / IN) for p in w.discretize(Deflection=0.005)]
            if pts and pts[0] != pts[-1]:
                pts.append(pts[0])
            circ = None
            es = w.Edges
            if es and all(type(e.Curve).__name__ == "Circle" for e in es):
                c0 = es[0].Curve
                if all((e.Curve.Center - c0.Center).Length < 1e-4 and abs(e.Curve.Radius - c0.Radius) < 1e-4 for e in es):
                    cc = c0.Center - o
                    circ = [cc.dot(u) / IN, cc.dot(v) / IN, c0.Radius / IN]
            loops.append({"pts": pts, "circle": circ})
        out["flat_loops"] = loops
    else:
        axes = [b["inner"]["axis"] for b in bends]
        if all(abs(abs(a.dot(axes[0])) - 1) < 1e-4 for a in axes):
            ax = axes[0]
            proj = [v.Point.dot(ax) for v in s.Vertexes]
            W = max(proj) - min(proj)
            gross = s.Volume / t + sum(math.pi * g["r"] ** 2 for g in holes)
            L = gross / W if W > 0 else 0
            out["flat_size"] = [max(W, L) / IN, min(W, L) / IN]
            out["flat_how"] = "estimated from volume/thickness/width: give the flat DXF for the exact size"
    return out


try:
    res = run()
except Exception as e:  # the report says why
    res = {"error": f"{type(e).__name__}: {e}", "trace": traceback.format_exc()[-1500:]}
with open(os.environ["SCS_STEP_OUT"], "w") as fh:
    json.dump(res, fh)
