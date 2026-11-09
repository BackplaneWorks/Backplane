"""Flat patterns for SendCutSend: write a clean DXF (R12, ASCII) from a part script, and develop bends.

Plain Python; inside FreeCAD, `face_dxf` writes a planar face's edges. Import it from a part script with
    import sys; sys.path.insert(0, "<this folder>"); import scs_sheet

    scs_sheet.write_dxf("mech/panel.dxf", units="mm",
        outline=[(0, 0), (200, 0), (200, 120), (0, 120)],      # closed polygon (or use rounded_rect / slot below)
        circles=[(20, 20, 2.5)],                               # (x, y, radius)
        loops=[scs_sheet.slot((100, 60), 30, 6, 0)],           # more closed loops: each a list of ("line", p, q) / ("arc", c, r, a0, a1)
        bends=[((0, 40), (200, 40))])                          # bend lines (layer BEND), edge to edge

Rules it keeps: one closed outline, holes and cutouts as closed loops, lines and arcs only (no splines),
no text, $INSUNITS set, bend lines alone on their own layer.
"""
import math

INSUNITS = {"in": 1, "mm": 4}


def bend_allowance(angle_deg, radius, thickness, k):
    """The developed length of one bend: the arc at the K-factor line."""
    return math.radians(angle_deg) * (radius + k * thickness)


def flat_length(flanges_outside, angle_deg, radius, thickness, k):
    """Flat length of a straight run of flanges given by their outside (formed) dimensions, all bends at angle_deg (<= 90)."""
    n = len(flanges_outside) - 1
    setback = math.tan(math.radians(angle_deg) / 2) * (radius + thickness)
    return sum(flanges_outside) - 2 * n * setback + n * bend_allowance(angle_deg, radius, thickness, k)


def poly_loop(pts):
    pts = list(pts)
    return [("line", pts[i], pts[(i + 1) % len(pts)]) for i in range(len(pts))]


def slot(c, length, width, angle_deg=0.0):
    """A stadium slot centred at c: overall length, width, along angle_deg."""
    r, h = width / 2, (length - width) / 2
    ux, uy = math.cos(math.radians(angle_deg)), math.sin(math.radians(angle_deg))
    a = (c[0] - ux * h, c[1] - uy * h)
    b = (c[0] + ux * h, c[1] + uy * h)
    nx, ny = -uy * r, ux * r
    ang = angle_deg
    return [("line", (a[0] - nx, a[1] - ny), (b[0] - nx, b[1] - ny)), ("arc", b, r, ang - 90, ang + 90),
            ("line", (b[0] + nx, b[1] + ny), (a[0] + nx, a[1] + ny)), ("arc", a, r, ang + 90, ang + 270)]


def rounded_rect(x0, y0, x1, y1, r):
    if r <= 0:
        return poly_loop([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
    return [("line", (x0 + r, y0), (x1 - r, y0)), ("arc", (x1 - r, y0 + r), r, 270, 360),
            ("line", (x1, y0 + r), (x1, y1 - r)), ("arc", (x1 - r, y1 - r), r, 0, 90),
            ("line", (x1 - r, y1), (x0 + r, y1)), ("arc", (x0 + r, y1 - r), r, 90, 180),
            ("line", (x0, y1 - r), (x0, y0 + r)), ("arc", (x0 + r, y0 + r), r, 180, 270)]


def _g(code, value):
    if isinstance(value, float):
        value = f"{value:.6f}"
    return f"{code}\n{value}\n"


def dxf_text(units="mm", outline=None, circles=(), loops=(), bends=(), layer="0", bend_layer="BEND"):
    out = [_g(0, "SECTION"), _g(2, "HEADER"), _g(9, "$ACADVER"), _g(1, "AC1009"), _g(9, "$INSUNITS"), _g(70, INSUNITS[units]), _g(0, "ENDSEC"),
           _g(0, "SECTION"), _g(2, "TABLES"), _g(0, "TABLE"), _g(2, "LAYER"), _g(70, 2)]
    for name, color in ((layer, 7), (bend_layer, 1)):
        out += [_g(0, "LAYER"), _g(2, name), _g(70, 0), _g(62, color), _g(6, "CONTINUOUS")]
    out += [_g(0, "ENDTAB"), _g(0, "ENDSEC"), _g(0, "SECTION"), _g(2, "ENTITIES")]

    def ent(e, lay):
        if e[0] == "line":
            (x0, y0), (x1, y1) = e[1], e[2]
            return [_g(0, "LINE"), _g(8, lay), _g(10, float(x0)), _g(20, float(y0)), _g(30, 0.0), _g(11, float(x1)), _g(21, float(y1)), _g(31, 0.0)]
        if e[0] == "arc":
            (cx, cy), r, a0, a1 = e[1], e[2], e[3], e[4]
            return [_g(0, "ARC"), _g(8, lay), _g(10, float(cx)), _g(20, float(cy)), _g(30, 0.0), _g(40, float(r)), _g(50, float(a0 % 360)), _g(51, float(a1 % 360 or 360))]
        raise ValueError(e[0])
    if outline:
        for e in (outline if isinstance(outline[0][0], str) else poly_loop(outline)):
            out += ent(e, layer)
    for x, y, r in circles:
        out += [_g(0, "CIRCLE"), _g(8, layer), _g(10, float(x)), _g(20, float(y)), _g(30, 0.0), _g(40, float(r))]
    for lp in loops:
        for e in (lp if isinstance(lp[0][0], str) else poly_loop(lp)):
            out += ent(e, layer)
    for p, q in bends:
        out += ent(("line", p, q), bend_layer)
    out += [_g(0, "ENDSEC"), _g(0, "EOF")]
    return "".join(out)


def write_dxf(path, **kw):
    with open(path, "w", newline="\n") as f:
        f.write(dxf_text(**kw))
    return path


def face_dxf(face, path, units="mm", bends=()):
    """Inside FreeCAD: a planar face (FreeCAD works in mm) as a DXF in its own plane: lines, arcs and circles kept, anything else as short lines."""
    import Part  # noqa: F401  (FreeCAD)
    n = face.normalAt(0, 0)
    o = face.Surface.Position if hasattr(face.Surface, "Position") else face.Vertexes[0].Point
    ref = face.Edges[0].Vertexes[0].Point.sub(o)
    u = ref.normalize() if ref.Length > 1e-9 else n.cross(n.__class__(1, 0, 0) if abs(n.x) < 0.9 else n.__class__(0, 1, 0)).normalize()
    v = n.cross(u)
    s = 1.0 if units == "mm" else 1 / 25.4
    to2 = lambda p: ((p.sub(o)).dot(u) * s, (p.sub(o)).dot(v) * s)
    loops = []
    for w in [face.OuterWire] + [w for w in face.Wires if not w.isSame(face.OuterWire)]:
        lp = []
        for e in w.Edges:
            c = e.Curve
            name = type(c).__name__
            if name == "Line" or name == "LineSegment":
                lp.append(("line", to2(e.Vertexes[0].Point), to2(e.Vertexes[-1].Point)))
            elif name == "Circle" and len(e.Vertexes) == 1:
                cc = to2(c.Center)
                lp.append(("arc", cc, c.Radius * s, 0, 360))
            elif name == "Circle":
                cc = to2(c.Center)
                p0, p1 = to2(e.Vertexes[0].Point), to2(e.Vertexes[-1].Point)
                pm = to2(e.valueAt((e.FirstParameter + e.LastParameter) / 2))
                ang = lambda p: math.degrees(math.atan2(p[1] - cc[1], p[0] - cc[0]))
                a0, a1, am = ang(p0), ang(p1), ang(pm)
                ccw = ((am - a0) % 360) < ((a1 - a0) % 360)
                lp.append(("arc", cc, c.Radius * s, a0, a1) if ccw else ("arc", cc, c.Radius * s, a1, a0))
            else:
                pts = [to2(p) for p in e.discretize(Deflection=0.01)]
                lp += [("line", pts[i], pts[i + 1]) for i in range(len(pts) - 1)]
        loops.append(lp)
    circles = []
    rest = []
    for lp in loops:
        if len(lp) == 1 and lp[0][0] == "arc" and abs((lp[0][4] - lp[0][3]) % 360) < 1e-9:
            circles.append((lp[0][1][0], lp[0][1][1], lp[0][2]))
        else:
            rest.append(lp)
    bend2 = [(to2(p), to2(q)) for p, q in bends]
    return write_dxf(path, units=units, outline=rest[0] if rest else None, circles=circles, loops=rest[1:], bends=bend2)
