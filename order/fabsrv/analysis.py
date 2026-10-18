"""Measuring the uploaded files: what the price is worked out from.

Runs as its own process (`python -m fabsrv.analysis <quote dir>`) under
resource limits (quoting.py), so a malformed or hostile file can only take
down that process. It reads <dir>/manifest.json and <dir>/files/, and writes
<dir>/features.json: per part, what the files say (board size and layers,
pads, holes, BOM lines; a solid's volume, box and area; a flat pattern's
size, cut length and holes), plus problems that stop the quote and warnings
that do not.
"""
import csv
import io
import json
import math
import pathlib
import re
import sys

# ---------------------------------------------------------------- gerbers

COORD = re.compile(r"([XYIJ])([+-]?\d*\.?\d+)")


class Gerber:
    """Enough RS-274X for sizes and pad counts: coordinates, D01/D02/D03, arcs, regions."""

    def __init__(self, text):
        self.text = text
        self.unit = 1.0  # to mm
        self.dec = (4, 6)
        self.function = ""
        self.points = []
        self.flashes = 0
        self.regions = 0
        self._parse()

    def _num(self, s, axis):
        if "." in s:
            v = float(s)
        else:
            v = int(s) / (10 ** self.dec[1])
        return v * self.unit

    def _parse(self):
        x = y = 0.0
        mode = 1  # 1 linear, 2 cw, 3 ccw
        in_region = False
        for m in re.finditer(r"%([^%]*)%|([^%*]+)\*", self.text):
            ext, word = m.group(1), m.group(2)
            if ext is not None:
                ext = ext.strip().rstrip("*")
                if ext.startswith("FS"):
                    f = re.search(r"X(\d)(\d)", ext)
                    if f:
                        self.dec = (int(f.group(1)), int(f.group(2)))
                elif ext.startswith("MOIN"):
                    self.unit = 25.4
                elif ext.startswith("MOMM"):
                    self.unit = 1.0
                elif ext.startswith("TF.FileFunction"):
                    self.function = ext[len("TF.FileFunction,"):]
                continue
            w = word.strip()
            if not w:
                continue
            if w.startswith("G04"):
                continue
            if w.startswith("%"):
                continue
            g = re.match(r"G0?([0-9]+)", w)
            if g:
                n = int(g.group(1))
                if n in (1, 2, 3):
                    mode = n
                elif n == 36:
                    in_region = True
                    self.regions += 1
                elif n == 37:
                    in_region = False
                elif n == 70:
                    self.unit = 25.4
                elif n == 71:
                    self.unit = 1.0
                w = w[g.end():]
                if not w:
                    continue
            if w.startswith("M02"):
                break
            c = dict((a, v) for a, v in COORD.findall(w))
            d = re.search(r"D0?([123])$", w)
            nx = self._num(c["X"], "X") if "X" in c else x
            ny = self._num(c["Y"], "Y") if "Y" in c else y
            op = int(d.group(1)) if d else (1 if c else 0)
            if op == 1:
                if mode in (2, 3) and ("I" in c or "J" in c):
                    cx = x + (self._num(c["I"], "I") if "I" in c else 0.0)
                    cy = y + (self._num(c["J"], "J") if "J" in c else 0.0)
                    r = math.hypot(x - cx, y - cy)
                    a0 = math.atan2(y - cy, x - cx)
                    a1 = math.atan2(ny - cy, nx - cx)
                    if mode == 3 and a1 <= a0:
                        a1 += 2 * math.pi
                    if mode == 2 and a1 >= a0:
                        a1 -= 2 * math.pi
                    if abs(a1 - a0) < 1e-9 and (nx, ny) == (x, y):
                        a1 = a0 + (2 * math.pi if mode == 3 else -2 * math.pi)
                    for k in range(17):
                        a = a0 + (a1 - a0) * k / 16
                        self.points.append((cx + r * math.cos(a), cy + r * math.sin(a)))
                else:
                    self.points.append((x, y))
                    self.points.append((nx, ny))
            elif op == 3 and not in_region:
                self.flashes += 1
            x, y = nx, ny

    def bbox(self):
        if not self.points:
            return None
        xs = [p[0] for p in self.points]
        ys = [p[1] for p in self.points]
        return min(xs), min(ys), max(xs), max(ys)


def layer_role(name, function):
    """copper:N (1 top .. outer), outline, paste:top|bottom, drill, or ''."""
    f = function.lower()
    n = name.lower()
    if f.startswith("copper"):
        m = re.search(r"copper,l(\d+)", f)
        return f"copper:{m.group(1) if m else '0'}"
    if f.startswith("profile"):
        return "outline"
    if f.startswith("paste"):
        return "paste:bottom" if "bot" in f else "paste:top"
    if re.search(r"(^|[-_.])f[._]cu\b|\.gtl$", n):
        return "copper:1"
    if re.search(r"(^|[-_.])b[._]cu\b|\.gbl$", n):
        return "copper:99"
    m = re.search(r"in(\d+)[._]cu\b|\.g(\d+)$|\.gp?(\d+)l?$", n)
    if m:
        return f"copper:{int(next(g for g in m.groups() if g)) + 1}"
    if re.search(r"edge[._]cuts|\.gm1$|\.gko$|\.gml$", n):
        return "outline"
    if re.search(r"f[._]paste|\.gtp$", n):
        return "paste:top"
    if re.search(r"b[._]paste|\.gbp$", n):
        return "paste:bottom"
    if n.endswith((".drl", ".xln", ".exc")) or ("drill" in n and n.endswith(".txt")):
        return "drill"
    return ""


def drills(text, name):
    """Excellon: holes per tool diameter (mm), and whether the file is plated."""
    unit = 1.0
    tools, use = {}, {}
    cur = None
    plated = None
    low = name.lower()
    if "npth" in low or "non-plated" in low or "-np" in low:
        plated = False
    elif "pth" in low or "plated" in low:
        plated = True
    for line in text.splitlines():
        s = line.strip()
        if not s:
            continue
        if s.startswith(";"):
            u = s.upper()
            if "NON_PLATED" in u or "NONPLATED" in u or ",NPTH" in u or "NPTH" in u:
                plated = False if plated is None else plated
            elif "PLATED" in u or ",PTH" in u:
                plated = True if plated is None else plated
            continue
        if s.startswith("INCH") or s == "M72":
            unit = 25.4
        elif s.startswith("METRIC") or s == "M71":
            unit = 1.0
        m = re.match(r"T(\d+)(?:F\d+S\d+)?C([\d.]+)", s)
        if m:
            tools[m.group(1)] = float(m.group(2)) * unit
            continue
        m = re.match(r"T(\d+)$", s)
        if m:
            cur = m.group(1)
            continue
        if cur and re.match(r"[XY]", s):
            d = tools.get(cur)
            if d:
                use[d] = use.get(d, 0) + 1
    return use, (True if plated is None else plated)


def board(files_root, gerbers):
    """A board's measures from its gerbers (a directory, or an unpacked .zip at <zip>.d)."""
    base = files_root / gerbers
    d = base.with_name(base.name + ".d") if base.suffix.lower() == ".zip" else base
    if not d.is_dir():
        return None, ["the gerbers are missing"]
    out = {"copper": [], "paste_top": 0, "paste_bottom": 0, "holes": 0, "min_drill_mm": None, "tht_holes": 0}
    problems = []
    outline = None
    for f in sorted(d.rglob("*")):
        if not f.is_file():
            continue
        text = f.read_text(errors="replace")
        if f.suffix.lower() in (".drl", ".xln", ".exc") or (text.lstrip().startswith("M48")):
            use, plated = drills(text, f.name)
            for dia, n in use.items():
                out["holes"] += n
                # the smallest plated hole sets the via tier (pricing.py)
                if plated and (out["min_drill_mm"] is None or dia < out["min_drill_mm"]):
                    out["min_drill_mm"] = dia
                if plated and 0.5 < dia < 2.5:
                    out["tht_holes"] += n
            continue
        if f.suffix.lower() == ".gbrjob":
            continue
        g = Gerber(text)
        role = layer_role(f.name, g.function)
        if role.startswith("copper"):
            out["copper"].append(int(role.split(":")[1]))
        elif role == "outline":
            outline = g.bbox() if outline is None else _join(outline, g.bbox())
        elif role == "paste:top":
            out["paste_top"] += g.flashes + g.regions
        elif role == "paste:bottom":
            out["paste_bottom"] += g.flashes + g.regions
    out["layers"] = len(out["copper"])
    del out["copper"]
    if outline is None:
        problems.append("no board outline (Edge.Cuts) among the gerbers")
    else:
        out["width_mm"] = round(outline[2] - outline[0], 2)
        out["height_mm"] = round(outline[3] - outline[1], 2)
    if out["layers"] == 0:
        problems.append("no copper layers among the gerbers")
    return out, problems


def _join(a, b):
    if b is None:
        return a
    return min(a[0], b[0]), min(a[1], b[1]), max(a[2], b[2]), max(a[3], b[3])


# ---------------------------------------------------------------- BOM and positions

def _rows(path):
    raw = path.read_bytes()[:5 * 1024 * 1024].decode("utf-8-sig", errors="replace")
    sample = raw[:4096]
    try:
        dialect = csv.Sniffer().sniff(sample, delimiters=",;\t")
    except csv.Error:
        dialect = csv.excel
    rows = list(csv.reader(io.StringIO(raw), dialect))
    rows = [r for r in rows if any(c.strip() for c in r)]
    if not rows:
        return [], []
    head = [h.strip().lower() for h in rows[0]]
    return head, rows[1:20001]


def _col(head, *names):
    for n in names:
        if n in head:
            return head.index(n)
    return None


RANGE = re.compile(r"^([A-Za-z_#]+)(\d+)-(?:[A-Za-z_#]+)?(\d+)$")


def refs_of(cell):
    """'C1,C3 C6-C8' as C1, C3, C6, C7, C8 (KiCad groups references into ranges)."""
    out = []
    for x in re.split(r"[,\s;]+", cell):
        if not x:
            continue
        m = RANGE.match(x)
        if m and int(m.group(3)) >= int(m.group(2)) and int(m.group(3)) - int(m.group(2)) < 500:
            out += [f"{m.group(1)}{i}" for i in range(int(m.group(2)), int(m.group(3)) + 1)]
        else:
            out.append(x)
    return out


def bom(path):
    """Lines of {refs, qty, mpn, maker, code}; problems for lines with no part number."""
    head, rows = _rows(path)
    ref = _col(head, "designator", "reference", "references", "ref", "refs")
    mpn = _col(head, "mpn", "manufacturer part number", "mfr part", "mfr. part #", "mfr.part", "manufacturer_part_number",
               "part number", "mfg part number", "mfr part number")
    maker = _col(head, "manufacturer", "mfr", "mfg", "maker")
    code = _col(head, "lcsc", "lcsc part", "lcsc part #", "jlcpcb part", "supplier part")
    dnp = _col(head, "dnp", "do not place", "exclude from bom")
    qtyc = _col(head, "qty", "quantity")
    if ref is None:
        return [], ["the BOM has no Designator/Reference column"]
    if mpn is None and code is None:
        return [], ["the BOM has no manufacturer part number (MPN) column"]
    lines, problems = [], []
    for r in rows:
        cell = lambda i: r[i].strip() if i is not None and i < len(r) else ""
        if dnp is not None and cell(dnp).lower() in ("1", "yes", "true", "x", "dnp"):
            continue
        refs = refs_of(cell(ref))
        if not refs:
            continue
        n = int(cell(qtyc)) if qtyc is not None and cell(qtyc).isdigit() and int(cell(qtyc)) > 0 else len(refs)
        m = cell(mpn)
        if not m and not cell(code):
            problems.append(f"{', '.join(refs[:4])}: no manufacturer part number")
            continue
        lines.append({"refs": refs[:200], "qty": n, "mpn": m[:80], "maker": cell(maker)[:60], "code": cell(code)[:20]})
    return lines[:2000], problems


def positions(path):
    """Placements per side, and the references placed."""
    head, rows = _rows(path)
    ref = _col(head, "designator", "ref", "reference")
    side = _col(head, "layer", "side")
    if ref is None:
        return None, ["the positions file has no Designator/Ref column"]
    top = bottom = 0
    refs = set()
    for r in rows:
        if ref >= len(r):
            continue
        refs.add(r[ref].strip())
        s = r[side].strip().lower() if side is not None and side < len(r) else "top"
        if s.startswith("b"):
            bottom += 1
        else:
            top += 1
    return {"top": top, "bottom": bottom, "refs": sorted(refs)[:5000]}, []


# ---------------------------------------------------------------- solids

def solid(path):
    """Volume (cm3), surface area (cm2) and the tightest box (mm) of a STEP file's solids."""
    from OCP.Bnd import Bnd_OBB
    from OCP.BRepBndLib import BRepBndLib
    from OCP.BRepGProp import BRepGProp
    from OCP.GProp import GProp_GProps
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.STEPControl import STEPControl_Reader

    r = STEPControl_Reader()
    if r.ReadFile(str(path)) != IFSelect_RetDone:
        return None, [f"{path.name} could not be read as STEP"]
    r.TransferRoots()
    shape = r.OneShape()
    if shape is None or shape.IsNull():
        return None, [f"{path.name} holds no solid"]
    vp, sp = GProp_GProps(), GProp_GProps()
    BRepGProp.VolumeProperties_s(shape, vp)
    BRepGProp.SurfaceProperties_s(shape, sp)
    obb = Bnd_OBB()
    BRepBndLib.AddOBB_s(shape, obb, True, True, True)
    dims = sorted([2 * obb.XHSize(), 2 * obb.YHSize(), 2 * obb.ZHSize()], reverse=True)
    vol = abs(vp.Mass()) / 1000.0
    if vol <= 0:
        return None, [f"{path.name} has no closed volume (export solids, not surfaces)"]
    return {"volume_cm3": round(vol, 3), "area_cm2": round(sp.Mass() / 100.0, 2),
            "size_x_mm": round(dims[0], 2), "size_y_mm": round(dims[1], 2), "size_z_mm": round(dims[2], 2)}, []


# ---------------------------------------------------------------- flat patterns

def dxf(path):
    """A flat pattern's box, total cut length and round holes, from a DXF's entities."""
    text = path.read_text(errors="replace")
    lines = text.splitlines()
    pairs = [(lines[i].strip(), lines[i + 1].strip()) for i in range(0, len(lines) - 1, 2)]
    scale = 1.0
    for i, (c, v) in enumerate(pairs):
        if c == "9" and v == "$INSUNITS" and i + 1 < len(pairs):
            u = pairs[i + 1][1]
            scale = {"1": 25.4, "2": 304.8, "4": 1.0, "5": 10.0, "6": 1000.0}.get(u, 1.0)
            break
    try:
        start = pairs.index(("2", "ENTITIES"))
    except ValueError:
        return None, [f"{path.name} has no ENTITIES section"]
    ents, cur = [], None
    for c, v in pairs[start + 1:]:
        if c == "0":
            if cur:
                ents.append(cur)
            if v == "ENDSEC":
                cur = None
                break
            cur = {"type": v, "g": []}
        elif cur is not None:
            cur["g"].append((c, v))
    pts, cut, holes = [], 0.0, 0

    def f(e, code, default=0.0):
        for c, v in e["g"]:
            if c == code:
                try:
                    return float(v)
                except ValueError:
                    return default
        return default

    poly = None
    for e in ents:
        t = e["type"]
        if t == "LINE":
            a, b = (f(e, "10"), f(e, "20")), (f(e, "11"), f(e, "21"))
            pts += [a, b]
            cut += math.dist(a, b)
        elif t == "CIRCLE":
            cx, cy, r = f(e, "10"), f(e, "20"), f(e, "40")
            pts += [(cx - r, cy - r), (cx + r, cy + r)]
            cut += 2 * math.pi * r
            holes += 1
        elif t == "ARC":
            cx, cy, r = f(e, "10"), f(e, "20"), f(e, "40")
            a0, a1 = math.radians(f(e, "50")), math.radians(f(e, "51"))
            if a1 < a0:
                a1 += 2 * math.pi
            cut += r * (a1 - a0)
            for k in range(17):
                a = a0 + (a1 - a0) * k / 16
                pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
        elif t == "LWPOLYLINE":
            vs, bulges = [], []
            closed = int(f(e, "70")) & 1
            x = None
            for c, v in e["g"]:
                if c == "10":
                    x = float(v)
                elif c == "20" and x is not None:
                    vs.append((x, float(v)))
                    bulges.append(0.0)
                elif c == "42" and bulges:
                    bulges[-1] = float(v)
            cut += _poly_len(vs, bulges, closed)
            pts += vs
        elif t == "POLYLINE":
            poly = {"vs": [], "bulges": [], "closed": int(f(e, "70")) & 1}
        elif t == "VERTEX" and poly is not None:
            poly["vs"].append((f(e, "10"), f(e, "20")))
            poly["bulges"].append(f(e, "42"))
        elif t == "SEQEND" and poly is not None:
            cut += _poly_len(poly["vs"], poly["bulges"], poly["closed"])
            pts += poly["vs"]
            poly = None
        elif t in ("SPLINE", "ELLIPSE"):
            xs = [(float(v), c) for c, v in e["g"] if c in ("10", "20")]
            ctrl = [(xs[i][0], xs[i + 1][0]) for i in range(0, len(xs) - 1, 2)]
            cut += sum(math.dist(ctrl[i], ctrl[i + 1]) for i in range(len(ctrl) - 1))
            pts += ctrl
    if not pts:
        return None, [f"{path.name} has no outline"]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return {"flat_x_mm": round((max(xs) - min(xs)) * scale, 2), "flat_y_mm": round((max(ys) - min(ys)) * scale, 2),
            "cut_length_mm": round(cut * scale, 1), "holes": holes}, []


def _poly_len(vs, bulges, closed):
    n = len(vs)
    total = 0.0
    for i in range(n if closed else n - 1):
        a, b = vs[i], vs[(i + 1) % n]
        chord = math.dist(a, b)
        bl = bulges[i] if i < len(bulges) else 0.0
        if bl:
            theta = 4 * math.atan(abs(bl))
            total += chord * theta / (2 * math.sin(theta / 2)) if theta else chord
        else:
            total += chord
    return total


# ---------------------------------------------------------------- the package

def analyse(root):
    m = json.loads((root / "manifest.json").read_text())
    files = root / "files"
    parts, problems, warnings = {}, [], []
    for p in m["parts"]:
        name, kind, roles = p["name"], p["kind"], p["files"]
        got, probs = {}, []
        if kind == "pcb":
            b, probs = board(files, roles["gerbers"])
            got.update(b or {})
            if p.get("assemble"):
                lines, bp = bom(files / roles["bom"])
                pos, pp = positions(files / roles["positions"])
                probs += bp + pp
                got["bom"] = lines
                if pos:
                    got["placed_top"], got["placed_bottom"] = pos["top"], pos["bottom"]
                    placed = set(pos["refs"])
                    missing = [r for ln in lines for r in ln["refs"] if r not in placed]
                    if missing:
                        warnings.append(f"{name}: {len(missing)} BOM parts have no position "
                                        f"(e.g. {', '.join(missing[:4])}); through-hole parts are fine")
        else:
            s, probs = solid(files / roles["step"])
            got.update(s or {})
            if kind == "sheetmetal" and "dxf" in roles:
                d, dp = dxf(files / roles["dxf"])
                probs += dp
                got.update(d or {})
        parts[name] = got
        problems += [f"{name}: {x}" for x in probs]
    return {"parts": parts, "problems": problems, "warnings": warnings}


def main():
    root = pathlib.Path(sys.argv[1])
    try:
        out = analyse(root)
    except Exception as e:  # the parent reads this; the files were bad, not the server
        out = {"parts": {}, "problems": [f"the files could not be read ({type(e).__name__})"], "warnings": []}
    (root / "features.json").write_text(json.dumps(out))


if __name__ == "__main__":
    main()
