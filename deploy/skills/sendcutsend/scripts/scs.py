#!/usr/bin/env python3
"""SendCutSend for agents: its live catalog and specs, DFM checks on a part, and the part's Backplane fab file.

  scs.py fetch [--refresh]                 fetch (or revalidate) the three sources and say how old each copy is
  scs.py status                            the cached copies and their ages, without the network
  scs.py guide                             the ordering guide (markdown)
  scs.py materials [--q TEXT] [--group G] [--category C] [--process P] [--service S] [--thickness 3mm|0.125in]
  scs.py material SKU                      one material: its catalog row and its specs, as JSON
  scs.py finishes [--type powder|anodizing|plating]
  scs.py hardware [--q TEXT] [--category C] [--sku SKU]
  scs.py measure PART [--bend-layer RE]    what the checks see of a part (DXF or STEP), as JSON
  scs.py dfm PART|FAB.json [--sku SKU] [--flat FLAT.dxf] [--service S]... [--finish TYPE:VALUE] [--bend-angle DEG]...
             [--hardware SKU]... [--tap DIA:THREAD]... [--countersink DIA:MAJOR]... [--json]
  scs.py fab PART --sku SKU [--flat FLAT.dxf] [--name N] [--qty Q] [same options as dfm] [-o OUT.fab.json]

The sources (never copied into this skill; fetched at use time, cached):
  guide   https://cdn.sendcutsend.com/specs/sendcutsend-ordering-guide.md
  catalog https://app.sendcutsend.com/sendcutsend-catalog.json
  specs   https://app.sendcutsend.com/sendcutsend-specs.json
Cache: $SCS_CACHE (default ~/.cache/backplane/sendcutsend). A copy younger than $SCS_TTL seconds (default 21600)
is used as is; an older one is revalidated (If-None-Match / If-Modified-Since); offline, or with SCS_OFFLINE=1,
the cached copy is used and its age is said. Dimensions in the specs are inches; this tool reports both.
Exit codes: 0 ok (dfm: no rule failed), 1 a dfm rule failed, 2 an error (no data, unreadable part).
"""
import argparse, datetime, hashlib, json, math, os, re, subprocess, sys, time, urllib.error, urllib.request
from pathlib import Path

SOURCES = {
    "guide": ("https://cdn.sendcutsend.com/specs/sendcutsend-ordering-guide.md", "sendcutsend-ordering-guide.md"),
    "catalog": ("https://app.sendcutsend.com/sendcutsend-catalog.json", "sendcutsend-catalog.json"),
    "specs": ("https://app.sendcutsend.com/sendcutsend-specs.json", "sendcutsend-specs.json"),
}
UA = "Backplane-sendcutsend-skill/1 (+https://github.com/BackplaneWorks/Backplane)"
MM = 25.4
HERE = Path(__file__).resolve().parent


class ScsError(Exception):
    pass


# ---------------------------------------------------------------- fetching and the cache

def cache_dir():
    d = os.environ.get("SCS_CACHE") or os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "backplane", "sendcutsend")
    return Path(d)


def ttl():
    try:
        return max(0, int(os.environ.get("SCS_TTL", "21600")))
    except ValueError:
        return 21600


def age_text(secs):
    secs = max(0, int(secs))
    if secs < 90:
        return f"{secs} s"
    if secs < 5400:
        return f"{secs // 60} min"
    if secs < 172800:
        return f"{secs / 3600:.1f} h"
    return f"{secs / 86400:.1f} days"


def stamp(t):
    return datetime.datetime.fromtimestamp(t, datetime.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")


def valid(name, data):
    """A fetched body is kept only when it is what it should be."""
    if not data:
        return "empty"
    if name == "guide":
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            return "not UTF-8 text"
        return ""
    try:
        o = json.loads(data)
    except ValueError:
        return "not JSON"
    if not isinstance(o, dict) or not isinstance(o.get("materials"), list) or not o["materials"]:
        return "no materials list"
    return ""


class Fetcher:
    """One process's view of the three sources. `opener`, `now` and `sleep` are for tests."""

    def __init__(self, cache=None, ttl_s=None, offline=None, opener=None, now=None, sleep=None, urls=None):
        self.cache = Path(cache) if cache else cache_dir()
        self.ttl = ttl() if ttl_s is None else ttl_s
        self.offline = (os.environ.get("SCS_OFFLINE") == "1") if offline is None else offline
        self.opener = opener or (lambda req, timeout: urllib.request.urlopen(req, timeout=timeout))
        self.now = now or time.time
        self.sleep = sleep or time.sleep
        self.urls = urls or {k: v[0] for k, v in SOURCES.items()}
        self.last_request = 0.0
        self.loaded = {}

    def paths(self, name):
        f = SOURCES[name][1]
        return self.cache / f, self.cache / (f + ".meta.json")

    def meta(self, name):
        body, meta = self.paths(name)
        try:
            m = json.loads(meta.read_text())
        except (OSError, ValueError):
            return None
        return m if body.is_file() else None

    def write(self, name, data, m):
        body, meta = self.paths(name)
        self.cache.mkdir(parents=True, exist_ok=True)
        if data is not None:
            tmp = body.with_suffix(body.suffix + ".part")
            tmp.write_bytes(data)
            os.replace(tmp, body)
        tmp = meta.with_suffix(".part")
        tmp.write_text(json.dumps(m, indent=1))
        os.replace(tmp, meta)

    def polite(self):
        # the guide asks automated clients to space their requests by at least a second
        wait = self.last_request + 1.0 - self.now()
        if wait > 0:
            self.sleep(wait)
        self.last_request = self.now()

    def get(self, name, refresh=False):
        """{name, url, path, data, fetched_at, checked_at, how, note}. how: cached (fresh within the TTL),
        revalidated (304), downloaded, stale (the network failed: the old copy). Raises ScsError with no copy at all."""
        if name in self.loaded and not refresh:
            return self.loaded[name]
        url = self.urls[name]
        body, _ = self.paths(name)
        m = self.meta(name)
        now = self.now()
        how, note = "", ""
        if m and not refresh and now - m.get("checked_at", 0) < self.ttl:
            how = "cached"
        elif self.offline:
            if not m:
                raise ScsError(f"{name}: offline (SCS_OFFLINE=1) and no cached copy in {self.cache}")
            how, note = "stale", "offline mode"
        else:
            headers = {"User-Agent": UA, "Accept-Encoding": "identity"}
            if m and m.get("etag"):
                headers["If-None-Match"] = m["etag"]
            if m and m.get("last_modified"):
                headers["If-Modified-Since"] = m["last_modified"]
            self.polite()
            try:
                r = self.opener(urllib.request.Request(url, headers=headers), 30)
                data = r.read()
                bad = valid(name, data)
                if bad:
                    raise ScsError(f"the server sent something that is {bad}")
                hd = r.headers
                m = {"url": url, "etag": hd.get("ETag") or "", "last_modified": hd.get("Last-Modified") or "", "fetched_at": now, "checked_at": now,
                     "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
                self.write(name, data, m)
                how = "downloaded"
            except urllib.error.HTTPError as e:
                if e.code == 304 and m:
                    m["checked_at"] = now
                    self.write(name, None, m)
                    how = "revalidated"
                elif m:
                    how, note = "stale", f"HTTP {e.code}"
                else:
                    raise ScsError(f"{name}: {url} answered HTTP {e.code} and there is no cached copy")
            except (urllib.error.URLError, OSError, ScsError) as e:
                why = getattr(e, "reason", None) or e
                if not m:
                    raise ScsError(f"{name}: could not fetch {url} ({why}) and there is no cached copy")
                how, note = "stale", str(why)
        data = body.read_bytes()
        out = {"name": name, "url": url, "path": str(body), "data": data, "fetched_at": m["fetched_at"], "checked_at": m["checked_at"], "how": how, "note": note,
               "age": now - m["fetched_at"]}
        self.loaded[name] = out
        return out

    def json(self, name):
        s = self.get(name)
        if "obj" not in s:
            s["obj"] = json.loads(s["data"])
        return s["obj"]

    def line(self, name):
        """How old the copy is, in one line: the thing every report starts with."""
        s = self.get(name)
        gen = ""
        if name != "guide":
            gen = (self.json(name).get("_meta") or {}).get("generated_at", "")
            gen = f", generated {gen[:16].replace('T', ' ')}Z" if gen else ""
        if s["how"] == "stale":
            return f"{SOURCES[name][1]}: OFFLINE, using the cached copy fetched {stamp(s['fetched_at'])} ({age_text(s['age'])} old; {s['note']}){gen}"
        return f"{SOURCES[name][1]}: fetched {stamp(s['fetched_at'])} ({age_text(s['age'])} ago, {s['how']}){gen}"


# ---------------------------------------------------------------- the catalog and specs

def num(v):
    """'0.014' -> 0.014; '+/-0.030 per foot' -> 0.030; 'N/A', '' -> None."""
    if v is None:
        return None
    if isinstance(v, (int, float)):
        return float(v)
    m = re.search(r"-?\d+(?:\.\d+)?", str(v))
    return float(m.group(0)) if m else None


def size(v):
    """'68 x 44' -> (68.0, 44.0), the long side first; None when absent."""
    if not v:
        return None
    ns = [float(x) for x in re.findall(r"\d+(?:\.\d+)?", str(v))]
    if len(ns) < 2:
        return None
    return (max(ns[0], ns[1]), min(ns[0], ns[1]))


class Data:
    def __init__(self, fetcher):
        self.f = fetcher
        self.catalog = fetcher.json("catalog")
        self.specs = fetcher.json("specs")
        self.mats = {m["sku"]: m for m in self.catalog.get("materials", [])}
        self.spec = {m["sku"]: m for m in self.specs.get("materials", [])}
        self.hw = {h["sku"]: h for h in self.catalog.get("hardware", [])}
        self.finishes = self.catalog.get("finish_options", [])

    def material(self, sku):
        m = self.mats.get(sku)
        if not m:
            near = [k for k in self.mats if k.split("-")[0] == sku.split("-")[0]][:8]
            raise ScsError(f"no material {sku} in the catalog" + (f" (same family: {', '.join(near)})" if near else "") + "; list them with `scs.py materials`")
        return m


def thickness_in(text):
    """'3mm' -> 0.118; '0.125in' / '0.125' / '.125"' -> 0.125."""
    t = str(text).strip().lower().replace('"', "in")
    v = num(t)
    if v is None:
        raise ScsError(f"not a thickness: {text}")
    return v / MM if t.endswith("mm") else v


# ---------------------------------------------------------------- reading a DXF

def dxf_pairs(path):
    raw = Path(path).read_bytes()
    if raw.startswith(b"AutoCAD Binary DXF"):
        raise ScsError(f"{path}: a binary DXF; export it as ASCII DXF")
    lines = raw.decode("utf-8", errors="replace").splitlines()
    out = []
    for i in range(0, len(lines) - 1, 2):
        try:
            out.append((int(lines[i].strip()), lines[i + 1].strip()))
        except ValueError:
            raise ScsError(f"{path}: not a DXF (line {i + 1}: {lines[i][:30]!r})")
    return out


UNITS = {0: None, 1: 1.0, 2: 12.0, 4: 1 / MM, 5: 10 / MM, 6: 1000 / MM}  # $INSUNITS -> inches per drawing unit
UNIT_NAMES = {0: "unitless", 1: "inches", 2: "feet", 4: "millimetres", 5: "centimetres", 6: "metres"}


def read_dxf(path):
    """{units, insunits, entities: [{type, layer, codes...}], blocks: n}."""
    pairs = dxf_pairs(path)
    insunits, section, ents, cur, i, blocks = None, None, [], None, 0, 0
    while i < len(pairs):
        c, v = pairs[i]
        if c == 0 and v == "SECTION" and i + 1 < len(pairs):
            section = pairs[i + 1][1]
            i += 2
            continue
        if c == 0 and v == "ENDSEC":
            section, cur = None, None
        elif section == "HEADER" and c == 9 and v == "$INSUNITS" and i + 1 < len(pairs):
            insunits = int(float(pairs[i + 1][1]))
            i += 2
            continue
        elif section == "BLOCKS" and c == 0 and v == "BLOCK":
            blocks += 1
        elif section == "ENTITIES":
            if c == 0:
                cur = {"type": v, "layer": "0", "codes": []}
                ents.append(cur)
            elif cur is not None:
                if c == 8:
                    cur["layer"] = v
                cur["codes"].append((c, v))
        i += 1
    return {"insunits": insunits, "entities": ents, "blocks": blocks}


def arc_pts(c, r, a0, a1, step=5.0):
    """Points from angle a0 to a1 (degrees, counter-clockwise)."""
    while a1 <= a0:
        a1 += 360.0
    n = max(2, int(math.ceil((a1 - a0) / step)) + 1)
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * k / (n - 1))), c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * k / (n - 1)))) for k in range(n)]


def bulge_piece(p, q, b):
    if abs(b) < 1e-12:
        return {"pts": [p, q], "kind": "line"}
    dx, dy = q[0] - p[0], q[1] - p[1]
    f = (1 - b * b) / (4 * b)
    c = (p[0] + dx / 2 - dy * f, p[1] + dy / 2 + dx * f)
    r = math.hypot(p[0] - c[0], p[1] - c[1])
    a0, a1 = math.degrees(math.atan2(p[1] - c[1], p[0] - c[0])), math.degrees(math.atan2(q[1] - c[1], q[0] - c[0]))
    if b > 0:
        pts = arc_pts(c, r, a0, a1)
    else:
        pts = list(reversed(arc_pts(c, r, a1, a0)))
    sweep = 4 * math.degrees(math.atan(abs(b)))
    return {"pts": pts, "kind": "arc", "c": c, "r": r, "sweep": sweep}


def entity_pieces(e):
    """A DXF entity's geometry as pieces {pts, kind[, c, r, sweep]} in drawing units, or a note."""
    t, cs = e["type"], e["codes"]
    g = {}
    for c, v in cs:
        g.setdefault(c, v)
    f = lambda c, d=0.0: float(g.get(c, d))
    if t == "LINE":
        return [{"pts": [(f(10), f(20)), (f(11), f(21))], "kind": "line"}]
    if t == "CIRCLE":
        c, r = (f(10), f(20)), f(40)
        return [{"pts": arc_pts(c, r, 0, 360), "kind": "circle", "c": c, "r": r, "sweep": 360.0}]
    if t == "ARC":
        c, r, a0, a1 = (f(10), f(20)), f(40), f(50), f(51)
        sweep = (a1 - a0) % 360 or 360
        return [{"pts": arc_pts(c, r, a0, a1), "kind": "arc", "c": c, "r": r, "sweep": sweep}]
    if t == "LWPOLYLINE":
        vs, closed = [], int(g.get(70, 0)) & 1
        for c, v in cs:
            if c == 10:
                vs.append([float(v), 0.0, 0.0])
            elif c == 20 and vs:
                vs[-1][1] = float(v)
            elif c == 42 and vs:
                vs[-1][2] = float(v)
        return poly_pieces([(x, y) for x, y, _ in vs], [b for _, _, b in vs], closed)
    if t == "ELLIPSE":
        c, mx, my, ratio = (f(10), f(20)), f(11), f(21), f(40, 1)
        t0, t1 = f(41), f(42, 2 * math.pi)
        while t1 <= t0:
            t1 += 2 * math.pi
        a, ang = math.hypot(mx, my), math.atan2(my, mx)
        n = 72
        pts = []
        for k in range(n + 1):
            u = t0 + (t1 - t0) * k / n
            x, y = a * math.cos(u), a * ratio * math.sin(u)
            pts.append((c[0] + x * math.cos(ang) - y * math.sin(ang), c[1] + x * math.sin(ang) + y * math.cos(ang)))
        return [{"pts": pts, "kind": "ellipse"}]
    if t == "SPLINE":
        # fit points when present, else the control polygon: an approximation, said as such in the report
        fit = [(float(v), None) for c, v in cs if c == 11]
        fy = [float(v) for c, v in cs if c == 21]
        ctl = [float(v) for c, v in cs if c == 10]
        cy = [float(v) for c, v in cs if c == 20]
        pts = list(zip([x for x, _ in fit], fy)) if fit and len(fit) == len(fy) else list(zip(ctl, cy))
        if int(g.get(70, 0)) & 1 and pts:
            pts.append(pts[0])
        return [{"pts": pts, "kind": "spline"}] if len(pts) >= 2 else []
    return []


def poly_pieces(vs, bulges, closed):
    out = []
    n = len(vs)
    for k in range(n if closed else n - 1):
        p, q = vs[k], vs[(k + 1) % n]
        if p != q:
            out.append(bulge_piece(p, q, bulges[k] if k < len(bulges) else 0.0))
    return out


def polyline_pieces(ents, k):
    """An old-style POLYLINE: its VERTEX entities follow it until SEQEND."""
    head, vs, bs = ents[k], [], []
    j = k + 1
    while j < len(ents) and ents[j]["type"] == "VERTEX":
        g = dict((c, v) for c, v in ents[j]["codes"])
        vs.append((float(g.get(10, 0)), float(g.get(20, 0))))
        bs.append(float(g.get(42, 0)))
        j += 1
    closed = int(dict(head["codes"]).get(70, 0)) & 1
    return poly_pieces(vs, bs, closed), j


# ---------------------------------------------------------------- geometry

def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def seg_pt(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    l2 = dx * dx + dy * dy
    t = 0.0 if l2 == 0 else max(0.0, min(1.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2))
    return math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy)


def cross(o, a, b):
    return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])


def seg_cross(a, b, c, d):
    d1, d2, d3, d4 = cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d)
    return ((d1 > 0) != (d2 > 0)) and ((d3 > 0) != (d4 > 0))


def seg_seg(a, b, c, d):
    if seg_cross(a, b, c, d):
        return 0.0
    return min(seg_pt(a, c, d), seg_pt(b, c, d), seg_pt(c, a, b), seg_pt(d, a, b))


def edges(pts):
    return list(zip(pts, pts[1:]))


def bbox(pts):
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return (min(xs), min(ys), max(xs), max(ys))


def poly_dist(p, q):
    """The smallest distance between two polylines (bounding boxes first)."""
    ba, bb = bbox(p), bbox(q)
    best = float("inf")
    for a1, a2 in edges(p):
        for b1, b2 in edges(q):
            d = seg_seg(a1, a2, b1, b2)
            if d < best:
                best = d
                if d == 0:
                    return 0.0
    return best


def area(pts):
    return 0.5 * sum(a[0] * b[1] - b[0] * a[1] for a, b in edges(pts))


def length(pts):
    return sum(dist(a, b) for a, b in edges(pts))


def inside(pt, poly):
    x, y, c = pt[0], pt[1], False
    for a, b in edges(poly):
        if (a[1] > y) != (b[1] > y) and x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]:
            c = not c
    return c


def hull(pts):
    ps = sorted(set(pts))
    if len(ps) < 3:
        return ps
    lo, hi = [], []
    for p in ps:
        while len(lo) >= 2 and cross(lo[-2], lo[-1], p) <= 0:
            lo.pop()
        lo.append(p)
    for p in reversed(ps):
        while len(hi) >= 2 and cross(hi[-2], hi[-1], p) <= 0:
            hi.pop()
        hi.append(p)
    return lo[:-1] + hi[:-1]


def min_rect(pts):
    """The smallest-area rectangle around the points: (long, short)."""
    h = hull(pts)
    if len(h) < 3:
        x0, y0, x1, y1 = bbox(pts)
        return (max(x1 - x0, y1 - y0), min(x1 - x0, y1 - y0))
    best = None
    for a, b in zip(h, h[1:] + h[:1]):
        L = dist(a, b)
        if L == 0:
            continue
        ux, uy = (b[0] - a[0]) / L, (b[1] - a[1]) / L
        us = [p[0] * ux + p[1] * uy for p in h]
        vs = [-p[0] * uy + p[1] * ux for p in h]
        w, d = max(us) - min(us), max(vs) - min(vs)
        if best is None or w * d < best[0] * best[1]:
            best = (w, d)
    return (max(best), min(best))


def ray_hit(o, d, segs):
    """Distance along the unit ray o+t*d to the first segment it meets, or None."""
    best = None
    for a, b in segs:
        ex, ey = b[0] - a[0], b[1] - a[1]
        den = d[0] * ey - d[1] * ex
        if abs(den) < 1e-15:
            continue
        t = ((a[0] - o[0]) * ey - (a[1] - o[1]) * ex) / den
        u = ((a[0] - o[0]) * d[1] - (a[1] - o[1]) * d[0]) / den
        if t > 1e-9 and -1e-9 <= u <= 1 + 1e-9 and (best is None or t < best):
            best = t
    return best


# ---------------------------------------------------------------- the part model

def chain(pieces, tol):
    """Pieces joined end to end into loops (closed) and chains (open)."""
    key = lambda p: (round(p[0] / tol), round(p[1] / tol))
    left = list(range(len(pieces)))
    loops, opens = [], []
    near = lambda a, b: dist(a, b) <= tol
    while left:
        i = left.pop(0)
        p = pieces[i]
        if p["kind"] == "circle" or (len(p["pts"]) > 2 and near(p["pts"][0], p["pts"][-1])):
            loops.append({"pts": p["pts"], "pieces": [p]})
            continue
        pts, parts = list(p["pts"]), [p]
        for _ in range(2):  # grow at the tail, then turn around and grow at the other end
            grown = True
            while grown and not (len(pts) > 2 and near(pts[0], pts[-1])):
                grown = False
                for j in left:
                    q = pieces[j]["pts"]
                    if near(q[0], pts[-1]):
                        pts += q[1:]
                    elif near(q[-1], pts[-1]):
                        pts += list(reversed(q))[1:]
                    else:
                        continue
                    parts.append(pieces[j])
                    left.remove(j)
                    grown = True
                    break
            if len(pts) > 2 and near(pts[0], pts[-1]):
                break
            pts.reverse()
        if len(pts) > 2 and near(pts[0], pts[-1]):
            pts[-1] = pts[0]
            loops.append({"pts": pts, "pieces": parts})
        else:
            opens.append({"pts": pts, "pieces": parts})
    return loops, opens


def describe_loop(lp, tol):
    """Adds what a loop is: a round hole (d), a slot (w, l), or another shape (its smallest rectangle)."""
    ps = lp["pieces"]
    arcs = [p for p in ps if p["kind"] in ("arc", "circle")]
    if ps and len(arcs) == len(ps):
        c, r = arcs[0]["c"], arcs[0]["r"]
        if all(dist(a["c"], c) <= tol and abs(a["r"] - r) <= tol for a in arcs) and abs(sum(a["sweep"] for a in arcs) - 360) < 1:
            lp["shape"], lp["d"], lp["c"] = "hole", 2 * r, c
            return lp
    lines = [p for p in ps if p["kind"] == "line"]
    if len(ps) == 4 and len(arcs) == 2 and len(lines) == 2 and abs(arcs[0]["r"] - arcs[1]["r"]) <= tol and all(abs(a["sweep"] - 180) < 1 for a in arcs):
        lp["shape"], lp["w"], lp["l"] = "slot", 2 * arcs[0]["r"], 2 * arcs[0]["r"] + dist(arcs[0]["c"], arcs[1]["c"])
        return lp
    L, S = min_rect(lp["pts"])
    lp["shape"], lp["w"], lp["l"] = "cutout", S, L
    return lp


def model_from_pieces(cut, bends, scale, notes, source, tol_in=0.001):
    """The flat part in inches: outer loop, inner loops (holes, slots, cutouts), islands, opens, bends."""
    sc = lambda pts: [(x * scale, y * scale) for x, y in pts]
    for p in cut:
        p["pts"] = sc(p["pts"])
        if "c" in p:
            p["c"], p["r"] = (p["c"][0] * scale, p["c"][1] * scale), p["r"] * scale
    loops, opens = chain(cut, tol_in)
    for lp in loops:
        lp["area"] = abs(area(lp["pts"]))
    loops.sort(key=lambda lp: -lp["area"])
    m = {"source": source, "units": "in", "notes": notes, "outer": None, "inner": [], "islands": [], "others": [], "opens": opens, "bends": []}
    if not loops:
        return m
    m["outer"] = describe_loop(loops[0], tol_in)
    rest = loops[1:]
    for lp in rest:
        describe_loop(lp, tol_in)
        if not inside(lp["pts"][0], m["outer"]["pts"]):
            m["others"].append(lp)
            continue
        depth = sum(1 for o in rest if o is not lp and o["area"] > lp["area"] and inside(lp["pts"][0], o["pts"]) and inside(o["pts"][0], m["outer"]["pts"]))
        (m["islands"] if depth % 2 else m["inner"]).append(lp)
    for b in bends:
        m["bends"].append({"a": (b[0][0] * scale, b[0][1] * scale), "b": (b[1][0] * scale, b[1][1] * scale)})
    return m


def measure_dxf(path, bend_layer=r"bend"):
    d = read_dxf(path)
    notes = {"insunits": d["insunits"], "text": [], "splines": 0, "blocks": [], "ignored": {}, "duplicates": 0, "approx": False}
    scale = UNITS.get(d["insunits"] or 0)
    bend_re = re.compile(bend_layer, re.I)
    mark_re = re.compile(r"etch|engrav|mark|dim|note|annot|title|border|construct", re.I)
    cut, bends, seen = [], [], set()
    ents, k = d["entities"], 0
    while k < len(ents):
        e = ents[k]
        t, layer = e["type"], e["layer"]
        if t == "POLYLINE":
            pieces, k = polyline_pieces(ents, k)
        else:
            pieces = entity_pieces(e)
            k += 1
        if t in ("TEXT", "MTEXT", "ATTRIB", "ATTDEF", "DIMENSION", "LEADER", "MLEADER"):
            if not mark_re.search(layer):
                notes["text"].append(f"{t} on layer {layer}")
            continue
        if t == "INSERT":
            notes["blocks"].append(dict(e["codes"]).get(2, "?"))
            continue
        if t in ("SPLINE", "ELLIPSE") and not bend_re.search(layer):
            notes["splines"] += 1
            notes["approx"] = notes["approx"] or t == "SPLINE"
        if not pieces:
            if t not in ("VERTEX", "SEQEND", "POINT"):
                notes["ignored"][t] = notes["ignored"].get(t, 0) + 1
            continue
        if bend_re.search(layer):
            for p in pieces:
                bends.append((p["pts"][0], p["pts"][-1]))
            continue
        if mark_re.search(layer):
            notes["ignored"][f"layer {layer}"] = notes["ignored"].get(f"layer {layer}", 0) + 1
            continue
        for p in pieces:
            if p["kind"] == "line":
                kk = tuple(sorted((tuple(round(v, 6) for v in p["pts"][0]), tuple(round(v, 6) for v in p["pts"][1]))))
                if kk in seen:
                    notes["duplicates"] += 1
                    continue
                seen.add(kk)
            cut.append(p)
    m = model_from_pieces(cut, bends, scale or 1.0, notes, str(path))
    m["kind"] = "dxf"
    m["units_known"] = scale is not None
    return m


def measure_step(path, freecad=None):
    """The STEP part as the FreeCAD probe sees it (scs_step.py), in inches."""
    fc = freecad or os.environ.get("BACKPLANE_FREECAD") or os.environ.get("FREECADCMD") or "freecadcmd"
    out = Path(os.environ.get("TMPDIR") or "/tmp") / f"scs-step-{os.getpid()}.json"
    env = dict(os.environ, SCS_STEP_IN=str(Path(path).resolve()), SCS_STEP_OUT=str(out))
    code = f"exec(open({str(HERE / 'scs_step.py')!r}).read())"
    try:
        r = subprocess.run([fc, "-c", code], env=env, capture_output=True, text=True, timeout=300)
    except FileNotFoundError:
        raise ScsError(f"FreeCAD's freecadcmd is needed to read a STEP ({fc} not found); or check the flat DXF instead")
    except subprocess.TimeoutExpired:
        raise ScsError(f"freecadcmd took over 300 s on {path}")
    try:
        raw = json.loads(out.read_text())
    except (OSError, ValueError):
        raise ScsError(f"the FreeCAD probe gave no result for {path}: {(r.stderr or r.stdout)[-600:]}")
    finally:
        try:
            out.unlink()
        except OSError:
            pass
    if raw.get("error"):
        raise ScsError(f"{path}: {raw['error']}")
    return raw


def measure(path, bend_layer=r"bend"):
    p = str(path).lower()
    if p.endswith(".dxf"):
        return measure_dxf(path, bend_layer)
    if p.endswith((".step", ".stp")):
        raw = measure_step(path)
        if raw.get("flat_loops"):
            pieces = [{"pts": [tuple(x) for x in lp["pts"]], "kind": "circle" if lp.get("circle") else "poly",
                       **({"c": tuple(lp["circle"][0:2]), "r": lp["circle"][2], "sweep": 360.0} if lp.get("circle") else {})} for lp in raw["flat_loops"]]
            m = model_from_pieces(pieces, [], 1.0, {"insunits": 1, "text": [], "splines": 0, "blocks": [], "ignored": {}, "duplicates": 0, "approx": False}, str(path))
        else:
            m = {"source": str(path), "units": "in", "notes": {}, "outer": None, "inner": [], "islands": [], "others": [], "opens": [], "bends": []}
        m["kind"] = "step"
        m["units_known"] = True
        m["step"] = raw
        return m
    raise ScsError(f"{path}: give a .dxf, .step or .stp")


def flat_size(m):
    if m.get("outer"):
        x0, y0, x1, y1 = bbox(m["outer"]["pts"])
        return (max(x1 - x0, y1 - y0), min(x1 - x0, y1 - y0)), "flat pattern"
    st = m.get("step") or {}
    if st.get("flat_size"):
        return tuple(st["flat_size"]), st.get("flat_how", "estimated")
    return None, ""


def cut_length(m):
    if m.get("outer"):
        return sum(length(lp["pts"]) for lp in [m["outer"]] + m["inner"] + m["islands"]) + sum(length(o["pts"]) for o in m["opens"])
    return (m.get("step") or {}).get("cut_length")


def holes_of(m):
    """Round holes [(d, centre or None, edge distance or None, centre-to-edge or None)] in inches."""
    if m.get("outer"):
        out = []
        for lp in m["inner"]:
            if lp.get("shape") == "hole":
                e = poly_dist(lp["pts"], m["outer"]["pts"])
                out.append({"d": lp["d"], "c": lp["c"], "edge": e, "cl_edge": e + lp["d"] / 2})
        return out
    return [{"d": h["d"], "c": None, "edge": h.get("edge"), "cl_edge": None if h.get("edge") is None else h["edge"] + h["d"] / 2} for h in (m.get("step") or {}).get("holes", [])]


def flange_lengths(m, bend):
    """Flat flange length on each side of a bend line: the shortest perpendicular run from the bend line to the outline or the next bend."""
    a, b = bend["a"], bend["b"]
    L = dist(a, b)
    if L == 0:
        return []
    u = ((b[0] - a[0]) / L, (b[1] - a[1]) / L)
    n = (-u[1], u[0])
    segs = edges(m["outer"]["pts"]) + [(o["a"], o["b"]) for o in m["bends"] if o is not bend]
    out = []
    for side in (1, -1):
        d = (n[0] * side, n[1] * side)
        best = None
        for k in range(1, 40):
            s = (a[0] + u[0] * L * k / 40, a[1] + u[1] * L * k / 40)
            if not inside(s, m["outer"]["pts"]) or any(inside(s, lp["pts"]) for lp in m["inner"]):
                continue
            h = ray_hit(s, d, segs)
            if h is not None and (best is None or h < best):
                best = h
        out.append(best)
    return out


# ---------------------------------------------------------------- the rules

class Report:
    def __init__(self, part, sku):
        self.part, self.sku, self.rows, self.head, self.services = part, sku, [], [], []

    def add(self, rule, status, text, source):
        self.rows.append({"rule": rule, "status": status, "text": text, "source": source})

    def count(self, s):
        return sum(1 for r in self.rows if r["status"] == s)

    def failed(self):
        return self.count("fail") > 0

    def text(self):
        w = max([len(r["rule"]) for r in self.rows] + [4])
        out = list(self.head)
        for r in self.rows:
            out.append(f"{r['status'].upper():5} {r['rule']:<{w}}  {r['text']}\n      {'':<{w}}  source: {r['source']}")
        out.append(f"{self.count('pass')} pass · {self.count('fail')} fail · {self.count('warn')} warn · {self.count('skip')} not checked")
        return "\n".join(out)

    def json(self):
        return {"part": self.part, "sku": self.sku, "sources": self.head[1:], "rules": self.rows,
                "pass": self.count("pass"), "fail": self.count("fail"), "warn": self.count("warn"), "skip": self.count("skip")}


def fmt(v):
    return "?" if v is None else f'{v:.3f}" ({v * MM:.2f} mm)'


SERVICE_SPECS = {"bending": "bending_specs", "countersinking": "countersinking_specs", "tapping": "tapping_specs", "hardware": "hardware_insertion_specs",
                 "deburring": "deburring_specs", "tumbling": "tumbling_specs", "engraving": "engraving_specs", "powder": "powder_coating_specs",
                 "anodizing": "anodizing_specs", "plating": "plating_specs", "dimple_forming": "dimple_forming_specs"}


def thread_major_in(t):
    """'M4x0.7' -> 0.157; '10-32' / '#10-32' -> 0.190; '1/4-20' -> 0.25; None when unreadable."""
    s = t.strip().upper().lstrip("#")
    m = re.match(r"M(\d+(?:\.\d+)?)", s)
    if m:
        return float(m.group(1)) / MM
    m = re.match(r"(\d+)/(\d+)", s)
    if m:
        return int(m.group(1)) / int(m.group(2))
    m = re.match(r"(\d+)-\d+", s)
    if m:
        return 0.060 + 0.013 * int(m.group(1))  # numbered screw sizes
    return None


def near_d(a, b, tol=0.003):
    return abs(a - b) <= tol


def dfm(m, data, sku, services=(), finish=None, bend_angles=(), hardware=(), taps=(), countersinks=(), flat=None):
    """Every rule, checked against the live catalog and specs. `flat`: the flat pattern's model when `m` is a bent STEP."""
    mat = data.material(sku)
    sp = data.spec.get(sku) or {}
    cut, bnd = sp.get("cutting_specs") or {}, sp.get("bending_specs") or {}
    gen = sp.get("general_specs") or {}
    t_in = num(mat.get("thickness")) or num(gen.get("thickness"))
    r = Report(m["source"], sku)
    r.head = [f"SendCutSend DFM · {Path(m['source']).name} · {sku} {mat.get('name')} {mat.get('thickness', '')}\" ({mat.get('cutting_process')})",
              data.f.line("catalog"), data.f.line("specs")]
    S = lambda key: f"specs {sku} {key}"
    C = f"catalog materials[{sku}]"
    G = "ordering guide, File Format Guidelines"
    services = list(dict.fromkeys(services))
    if finish:
        services.append(finish[0])
    for want, svc in ((m.get("bends") or (flat or {}).get("bends") or (m.get("step") or {}).get("bends"), "bending"), (hardware, "hardware"), (taps, "tapping"), (countersinks, "countersinking")):
        if want and svc not in services:
            services.append(svc)
    r.services = services
    plan = flat or m  # the flat pattern, where outline rules are measured

    # material, stock, services, finish
    r.add("material", "fail" if mat.get("out_of_stock") else "pass", f"{sku} is in the catalog" + (" but OUT OF STOCK: pick another material or thickness" if mat.get("out_of_stock") else ", in stock"), C + ".out_of_stock")
    have = set(mat.get("available_services") or [])
    for s in services:
        r.add(f"service {s}", "pass" if s in have else "fail", f"{s} is offered on {sku}" if s in have else f"{sku} does not offer {s} (it offers: {', '.join(sorted(have)) or 'none'})", C + ".available_services")
    if finish:
        ok = [f for f in data.finishes if f.get("finish_type") == finish[0] and (not finish[1] or f.get("value") == finish[1])]
        r.add("finish", "pass" if ok else "fail", f"{finish[0]} {finish[1] or ''}".strip() + (" is a finish option" if ok else " is no finish option (see `scs.py finishes`)"), "catalog finish_options")

    # thickness of a modelled part
    st = m.get("step") or {}
    if st.get("thickness") and t_in:
        tp, tn = num(gen.get("thickness_tolerance_positive")) or 0.0, num(gen.get("thickness_tolerance_negative")) or 0.0
        ok = t_in - tn - 0.0005 <= st["thickness"] <= t_in + tp + 0.0005
        r.add("thickness", "pass" if ok else "fail", f"model {fmt(st['thickness'])}, material {fmt(t_in)} (+{tp}/-{tn})", S("general_specs.thickness, thickness_tolerance_*"))

    # file rules (DXF)
    if m["kind"] == "dxf" or (flat and flat["kind"] == "dxf"):
        dm = m if m["kind"] == "dxf" else flat
        nt = dm["notes"]
        iu = nt["insunits"]
        if not dm["units_known"]:
            r.add("units", "fail", f"$INSUNITS is {iu if iu is not None else 'missing'} ({UNIT_NAMES.get(iu or 0, 'unknown')}): set it to 1 (inches) or 4 (mm); checked as inches", G + " (Set $INSUNITS correctly)")
        else:
            r.add("units", "pass" if iu in (1, 4) else "warn", f"$INSUNITS {iu} ({UNIT_NAMES.get(iu, '?')})" + ("" if iu in (1, 4) else ": the upload asks INCH or MM; prefer 1 or 4"), G)
        r.add("no text", "fail" if nt["text"] else "pass", ("text as a font: " + ", ".join(nt["text"][:4]) + ": convert text to outlines") if nt["text"] else "no TEXT/MTEXT/DIMENSION on cut layers", G + " (convert text to outlines)")
        r.add("no blocks", "fail" if nt["blocks"] else "pass", ("block references " + ", ".join(nt["blocks"][:4]) + ": explode them before export") if nt["blocks"] else "no block references", "skill (a block's geometry is not on the cut layers)")
        r.add("curves", "warn" if nt["splines"] else "pass", f"{nt['splines']} spline/ellipse entities: use polylines and arcs where possible" + (" (measured from their control points: approximate)" if nt["approx"] else "") if nt["splines"] else "lines, arcs and polylines only", G + " (polylines and arcs, not splines)")
        if nt["duplicates"]:
            r.add("double lines", "warn", f"{nt['duplicates']} duplicate line(s) on top of each other: they are cut twice", "skill (overlapping entities)")
        if nt["ignored"]:
            r.add("ignored", "warn", "not read as cut geometry: " + ", ".join(f"{k} ×{v}" for k, v in nt["ignored"].items()), "skill (check these are not meant to be cut)")

    # contours
    if plan.get("outer") is not None or plan["kind"] == "dxf":
        if plan.get("outer") is None:
            r.add("closed contours", "fail", "no closed outline found", "skill (every cut path must close)")
            return r
        r.add("closed contours", "fail" if plan["opens"] else "pass", (f"{len(plan['opens'])} open path(s), first near " + ", ".join(f"{v:.3f}" for v in plan["opens"][0]["pts"][0]) + ": close them (gaps over 0.001\")") if plan["opens"] else "every cut path closes", "skill (an open path has no inside to cut out)")
        r.add("one part", "fail" if plan["others"] else "pass", f"{len(plan['others']) + 1} separate outlines: one part per file" if plan["others"] else "one outline", "skill (SendCutSend quotes each file as one part)")
        if plan["islands"]:
            r.add("part-in-part", "warn", f"{len(plan['islands'])} closed shape(s) inside a cutout: they fall out as loose pieces; send each as its own part if you want it", "skill (part-in-part)")

    # size
    fs, how = flat_size(plan)
    mn, mx_i, mx_c = size(cut.get("min_part_size") or mat.get("min_part_size")), size(cut.get("max_instant_pricing_part_size") or mat.get("max_instant_pricing_part_size")), size(cut.get("max_custom_quote_part_size") or mat.get("max_custom_quote_part_size"))
    if fs:
        tag = f'{fs[0]:.3f}" x {fs[1]:.3f}" ({fs[0] * MM:.1f} x {fs[1] * MM:.1f} mm, {how})'
        if mn and (fs[0] < mn[0] or fs[1] < mn[1]):
            r.add("min size", "fail", f"{tag} is under {mn[0]}\" x {mn[1]}\"", S("cutting_specs.min_part_size"))
        else:
            r.add("min size", "pass", f"{tag} ≥ {mn[0]}\" x {mn[1]}\"" if mn else tag, S("cutting_specs.min_part_size"))
        fits = lambda lim: lim and fs[0] <= lim[0] and fs[1] <= lim[1]
        if fits(mx_i):
            r.add("max size", "pass", f"{tag} ≤ {mx_i[0]}\" x {mx_i[1]}\" (instant pricing)", S("cutting_specs.max_instant_pricing_part_size"))
        elif fits(mx_c):
            r.add("max size", "warn", f"{tag} is over {mx_i[0]}\" x {mx_i[1]}\": custom quote only (up to {mx_c[0]}\" x {mx_c[1]}\")", S("cutting_specs.max_custom_quote_part_size"))
        else:
            r.add("max size", "fail", f"{tag} is over {(mx_c or mx_i or ('?', '?'))[0]}\" x {(mx_c or mx_i or ('?', '?'))[1]}\"", S("cutting_specs.max_custom_quote_part_size"))
        for s in services:
            lim = sp.get(SERVICE_SPECS.get(s, ""), {})
            lo, hi = size(lim.get("min_flat_part_size")), size(lim.get("max_flat_part_size"))
            if lo or hi:
                ok = (not lo or (fs[0] >= lo[0] and fs[1] >= lo[1])) and (not hi or (fs[0] <= hi[0] and fs[1] <= hi[1]))
                r.add(f"{s} size", "pass" if ok else "fail", f"{tag} within {lim.get('min_flat_part_size', '-')} .. {lim.get('max_flat_part_size', '-')} in", S(f"{SERVICE_SPECS[s]}.min/max_flat_part_size"))
    else:
        r.add("max size", "skip", "no flat size measured: give the flat DXF (--flat)", S("cutting_specs.max_instant_pricing_part_size"))

    # holes, slots, webs (flat pattern)
    min_hole, min_bridge, min_edge = num(cut.get("min_hole_size")), num(cut.get("min_bridge_size")), num(cut.get("min_hole_to_edge"))
    if plan.get("outer"):
        holes = [lp for lp in plan["inner"] if lp["shape"] == "hole"]
        other = [lp for lp in plan["inner"] if lp["shape"] != "hole"]
        if holes and min_hole:
            h = min(holes, key=lambda lp: lp["d"])
            r.add("min hole", "pass" if h["d"] >= min_hole else "fail", f"smallest hole Ø{fmt(h['d'])} {'≥' if h['d'] >= min_hole else '<'} {fmt(min_hole)} ({len(holes)} holes)", S("cutting_specs.min_hole_size"))
        if other and min_hole:
            s = min(other, key=lambda lp: lp["w"])
            r.add("min slot", "pass" if s["w"] >= min_hole else "fail", f"narrowest {s['shape']} {fmt(s['w'])} wide {'≥' if s['w'] >= min_hole else '<'} {fmt(min_hole)} ({len(other)} slots/cutouts)", S("cutting_specs.min_hole_size (applied to slot width)"))
        inner = plan["inner"]
        if inner and min_edge is not None:
            e = min(((poly_dist(lp["pts"], plan["outer"]["pts"]), lp) for lp in inner), key=lambda x: x[0])
            r.add("hole to edge", "pass" if e[0] >= min_edge else "fail", f"closest feature to the outline {fmt(e[0])} {'≥' if e[0] >= min_edge else '<'} {fmt(min_edge)}", S("cutting_specs.min_hole_to_edge"))
        if len(inner) > 1 and min_bridge is not None:
            best = None
            for i in range(len(inner)):
                for j in range(i + 1, len(inner)):
                    d = poly_dist(inner[i]["pts"], inner[j]["pts"])
                    if best is None or d < best:
                        best = d
            r.add("min web", "pass" if best >= min_bridge else "fail", f"narrowest web between features {fmt(best)} {'≥' if best >= min_bridge else '<'} {fmt(min_bridge)}", S("cutting_specs.min_bridge_size"))
    elif st.get("holes"):
        hs = st["holes"]
        h = min(hs, key=lambda x: x["d"])
        if min_hole:
            r.add("min hole", "pass" if h["d"] >= min_hole else "fail", f"smallest hole Ø{fmt(h['d'])} {'≥' if h['d'] >= min_hole else '<'} {fmt(min_hole)} ({len(hs)} holes, from the 3D model)", S("cutting_specs.min_hole_size"))
        es = [x["edge"] for x in hs if x.get("edge") is not None]
        if es and min_edge is not None:
            r.add("hole to edge", "pass" if min(es) >= min_edge else "fail", f"closest hole to an edge {fmt(min(es))} (3D) vs {fmt(min_edge)}", S("cutting_specs.min_hole_to_edge"))
        ws = [x for x in st.get("hole_webs", [])]
        if ws and min_bridge is not None:
            r.add("min web", "pass" if min(ws) >= min_bridge else "fail", f"narrowest web between holes {fmt(min(ws))} (3D) vs {fmt(min_bridge)}", S("cutting_specs.min_bridge_size"))

    # bends
    want_bends = bool(plan.get("bends")) or bool(st.get("bends"))
    if want_bends and not bnd:
        r.add("bending", "fail", f"the part has bends but {sku} has no bending specs: {mat.get('name')} {mat.get('thickness')}\" is not bent", S("bending_specs"))
    elif want_bends:
        fb, fa = num(bnd.get("min_flange_length_before_bend")), num(bnd.get("min_flange_length_after_bend"))
        rad, k, maxlen = num(bnd.get("effective_bend_radius") or bnd.get("bend_radius")), num(bnd.get("k_factor")), num(bnd.get("max_bend_length"))
        amax, amin = num(bnd.get("max_bend_angle")), num(bnd.get("min_bend_angle"))
        if plan.get("bends"):
            angles = list(bend_angles) or [90.0]
            for i, b in enumerate(plan["bends"]):
                ang = angles[i] if i < len(angles) else angles[-1]
                L = dist(b["a"], b["b"])
                tag = f"bend {i + 1}"
                reach = [inside(p, plan["outer"]["pts"]) and seg_pt_poly(p, plan["outer"]["pts"]) > 0.002 for p in (b["a"], b["b"])]
                r.add(f"{tag} spans", "fail" if any(reach) else "pass", "the bend line stops inside the part: it must run fully across the area to be bent" if any(reach) else "the bend line runs edge to edge", "ordering guide, Bending (bend lines must extend completely across)")
                if maxlen:
                    r.add(f"{tag} length", "pass" if L <= maxlen else "fail", f"{fmt(L)} {'≤' if L <= maxlen else '>'} {fmt(maxlen)}", S("bending_specs.max_bend_length"))
                if amax:
                    ok = ang <= amax and (not amin or ang >= amin)
                    r.add(f"{tag} angle", "pass" if ok else "fail", f"{ang:g}° within {amin or 0:g}..{amax:g}°", S("bending_specs.min/max_bend_angle"))
                if fb:
                    fl = [x for x in flange_lengths(plan, b) if x is not None]
                    if fl:
                        f = min(fl)
                        r.add(f"{tag} flange", "pass" if f >= fb - 1e-4 else "fail", f"shortest flat flange {fmt(f)} {'≥' if f >= fb - 1e-4 else '<'} {fmt(fb)}", S("bending_specs.min_flange_length_before_bend"))
                if fb:
                    near = []
                    for lp in plan["inner"]:
                        d = poly_dist(lp["pts"], [b["a"], b["b"]])
                        if d > 1e-4:
                            near.append((d, lp))
                    if near:
                        d, lp = min(near, key=lambda x: x[0])
                        r.add(f"{tag} hole distance", "pass" if d >= fb - 1e-4 else "fail", f"closest {lp['shape']} edge {fmt(d)} from the bend line {'≥' if d >= fb - 1e-4 else '<'} {fmt(fb)}" + ("" if d >= fb - 1e-4 else ": it sits over the die and distorts; move it out or cut a relief through the bend"),
                              S("bending_specs.min_flange_length_before_bend (features within the minimum flange distort)"))
            if rad is not None:
                r.add("bend radius", "info" if m["kind"] == "dxf" and not flat else "pass", f"SendCutSend bends {sku} at an inside radius of {fmt(rad)}, K-factor {k}, bend deduction {bnd.get('bend_deduction')}\": the flat pattern must be developed with these", S("bending_specs.effective_bend_radius, k_factor, bend_deduction"))
        if st.get("bends"):
            for i, b in enumerate(st["bends"]):
                tag = f"bend {i + 1}"
                if rad is not None:
                    ok = abs(b["radius"] - rad) <= max(0.01, 0.25 * rad)
                    r.add(f"{tag} radius", "pass" if ok else "warn", f"modelled inside radius {fmt(b['radius'])}, SendCutSend's {fmt(rad)}" + ("" if ok else ": the formed part will differ; model the bend at their radius"), S("bending_specs.effective_bend_radius"))
                if amax:
                    ok = b["angle"] <= amax + 0.5 and (not amin or b["angle"] >= amin - 0.5)
                    r.add(f"{tag} angle", "pass" if ok else "fail", f"{b['angle']:.1f}° within {amin or 0:g}..{amax:g}°", S("bending_specs.min/max_bend_angle"))
                if maxlen:
                    r.add(f"{tag} length", "pass" if b["length"] <= maxlen else "fail", f"{fmt(b['length'])} vs {fmt(maxlen)}", S("bending_specs.max_bend_length"))
                ba = math.radians(b["angle"]) * (b["radius"] + (k or 0.42) * (st.get("thickness") or t_in or 0))
                if fb and b.get("flanges") and not flat:
                    f = min(b["flanges"]) + ba / 2
                    r.add(f"{tag} flange", "pass" if f >= fb - 1e-4 else "fail", f"shortest flange {fmt(f)} in the flat (straight run {fmt(min(b['flanges']))} + half the bend) {'≥' if f >= fb - 1e-4 else '<'} {fmt(fb)}", S("bending_specs.min_flange_length_before_bend"))
                if fa and b.get("flanges"):
                    f = min(b["flanges"]) + b["radius"] + (st.get("thickness") or t_in or 0)
                    if b["angle"] >= 85:
                        r.add(f"{tag} formed flange", "pass" if f >= fa - 1e-4 else "fail", f"shortest formed flange {fmt(f)} (outside) {'≥' if f >= fa - 1e-4 else '<'} {fmt(fa)}", S("bending_specs.min_flange_length_after_bend"))
                if fb and b.get("hole_dist") is not None and not flat:
                    d = b["hole_dist"] + ba / 2
                    r.add(f"{tag} hole distance", "pass" if d >= fb - 1e-4 else "fail", f"closest hole edge {fmt(d)} from the bend line in the flat {'≥' if d >= fb - 1e-4 else '<'} {fmt(fb)}", S("bending_specs.min_flange_length_before_bend (features within the minimum flange distort)"))

    # hardware, taps, countersinks
    hs = holes_of(plan) if plan.get("outer") else holes_of(m)
    for hsku in hardware:
        hw = data.hw.get(hsku)
        if not hw:
            r.add(f"hardware {hsku}", "fail", f"no hardware {hsku} in the catalog (see `scs.py hardware`)", "catalog hardware")
            continue
        ms = hw.get("manufacturing_specs") or {}
        lo, hi = num(ms.get("min_sheet_thickness")), num(ms.get("max_sheet_thickness"))
        if t_in and (lo or hi):
            ok = (lo is None or t_in >= lo - 1e-4) and (hi is None or t_in <= hi + 1e-4)
            r.add(f"hardware {hsku} sheet", "pass" if ok else "fail", f"{t_in}\" sheet within {lo}..{hi}\"", f"catalog hardware[{hsku}].manufacturing_specs.min/max_sheet_thickness")
        hd, ce = num(hw.get("hole_diameter")), num(hw.get("min_cl_to_edge_distance"))
        match = [h for h in hs if hd and near_d(h["d"], hd)]
        if not match:
            r.add(f"hardware {hsku} hole", "fail", f"no hole of Ø{fmt(hd)} for it", f"catalog hardware[{hsku}].hole_diameter")
        elif ce is not None:
            worst = min((h["cl_edge"] for h in match if h["cl_edge"] is not None), default=None)
            if worst is not None:
                r.add(f"hardware {hsku} edge", "pass" if worst >= ce - 1e-4 else "fail", f"{len(match)} hole(s); closest centre to edge {fmt(worst)} vs {fmt(ce)}", f"catalog hardware[{hsku}].min_cl_to_edge_distance")
    tsp = sp.get("tapping_specs") or {}
    for d, thread in taps:
        tag = f"tap {thread}"
        major, lo, hi = thread_major_in(thread), thread_major_in(tsp.get("smallest_tap") or ""), thread_major_in(tsp.get("largest_tap") or "")
        if major and lo and hi:
            r.add(f"{tag} size", "pass" if lo - 1e-4 <= major <= hi + 1e-4 else "fail", f"{thread} within {tsp.get('smallest_tap')}..{tsp.get('largest_tap')}", S("tapping_specs.smallest_tap, largest_tap"))
        match = [h for h in hs if near_d(h["d"], d)]
        if not match:
            r.add(f"{tag} hole", "fail", f"no hole of Ø{fmt(d)} to tap", "skill (the tap goes in a hole of the part)")
            continue
        ce = num(tsp.get("min_distance_cl_hole_to_edge"))
        e2 = num(tsp.get("min_hole_to_edge"))
        worst_cl = min((h["cl_edge"] for h in match if h["cl_edge"] is not None), default=None)
        worst_e = min((h["edge"] for h in match if h["edge"] is not None), default=None)
        if ce is not None and worst_cl is not None:
            r.add(f"{tag} edge", "pass" if worst_cl >= ce - 1e-4 else "fail", f"closest centre to edge {fmt(worst_cl)} vs {fmt(ce)}", S("tapping_specs.min_distance_cl_hole_to_edge"))
        elif e2 is not None and worst_e is not None:
            r.add(f"{tag} edge", "pass" if worst_e >= e2 - 1e-4 else "fail", f"closest hole to edge {fmt(worst_e)} vs {fmt(e2)}", S("tapping_specs.min_hole_to_edge"))
    csp = sp.get("countersinking_specs") or {}
    for d, major in countersinks:
        tag = f"countersink Ø{d:.3f}\""
        if not csp:
            r.add(tag, "fail", f"{sku} has no countersinking specs", S("countersinking_specs"))
            continue
        lo, hi, ce = num(csp.get("countersink_min_minor")), num(csp.get("countersink_max_major")), num(csp.get("min_hole_center_to_material_edge"))
        if lo is not None:
            r.add(f"{tag} minor", "pass" if d >= lo - 1e-4 else "fail", f"hole {fmt(d)} vs smallest minor {fmt(lo)}", S("countersinking_specs.countersink_min_minor"))
        if hi is not None and major:
            r.add(f"{tag} major", "pass" if major <= hi + 1e-4 else "fail", f"major {fmt(major)} vs largest {fmt(hi)}", S("countersinking_specs.countersink_max_major"))
        match = [h for h in hs if near_d(h["d"], d)]
        worst = min((h["cl_edge"] for h in match if h["cl_edge"] is not None), default=None)
        if not match:
            r.add(f"{tag} hole", "fail", f"no hole of Ø{fmt(d)} to countersink", "skill (the countersink goes in a hole of the part)")
        elif ce is not None and worst is not None:
            r.add(f"{tag} edge", "pass" if worst >= ce - 1e-4 else "fail", f"closest centre to edge {fmt(worst)} vs {fmt(ce)}", S("countersinking_specs.min_hole_center_to_material_edge"))
    return r


def seg_pt_poly(p, poly):
    return min(seg_pt(p, a, b) for a, b in edges(poly))


# ---------------------------------------------------------------- the fab file

# SendCutSend's materials and finishes that Backplane's order catalog (order/fabsrv/catalog.py) has an equivalent for.
SHEET = [(r"^5052", "aluminum_5052"), (r"304", "stainless_304"), (r"201", "stainless_201"), (r"Galvanized|G90", "steel_galvanized"), (r"A36/1008|1008|Mild Steel|Cold Rolled", "steel_cold_rolled")]
ROUTER = [(r"^Acrylic", "pmma"), (r"Polycarbonate", "polycarbonate"), (r"ABS Black", "abs_black"), (r"ABS White", "abs_white"), (r"Delrin|Acetal", "pom_black"), (r"G10|G-10|FR-?4", "fr4")]
FINISH = {"powder": "powder_coat", "anodizing": "anodize"}


def map_options(mat, finish, data):
    """(kind, options, unmapped): Backplane's fab kind and options for a SendCutSend material, and what has no equivalent."""
    grade = mat.get("grade") or mat.get("name") or ""
    t_in = num(mat.get("thickness")) or 0
    unmapped = []
    if mat.get("group") == "Metals":
        kind = "sheetmetal"
        m = next((v for pat, v in SHEET if re.search(pat, grade, re.I)), None)
        opts = {"material": m} if m else {}
        if not m:
            unmapped.append(f"material {grade}")
        tmm = round(t_in * MM, 2)
        if 0.5 <= tmm <= 6.0:
            opts["thickness_mm"] = tmm
        else:
            unmapped.append(f"thickness {t_in}\"")
        fin = "none"
        if finish:
            if finish[0] == "anodizing":
                f = next((f for f in data.finishes if f.get("finish_type") == "anodizing" and f.get("value") == finish[1]), {})
                fin = "hard_anodize" if "III" in (f.get("code") or "") else "anodize"
            elif finish[0] in FINISH:
                fin = FINISH[finish[0]]
            else:
                unmapped.append(f"finish {finish[0]} {finish[1] or ''}".strip())
        opts["finish"] = fin
        return kind, opts, unmapped
    kind = "cnc"
    m = next((v for pat, v in ROUTER if re.search(pat, grade, re.I)), None)
    if m == "pom_black" and re.search(r"white", mat.get("name", ""), re.I):
        m = "pom_white"
    opts = {"material": m, "finish": "none", "tolerance": "standard"} if m else {"finish": "none", "tolerance": "standard"}
    if not m:
        unmapped.append(f"material {grade}")
    if finish:
        unmapped.append(f"finish {finish[0]} {finish[1] or ''}".strip())
    return kind, opts, unmapped


def features(kind, m, flat, t_in):
    plan = flat or m
    fs, _ = flat_size(plan)
    cl = cut_length(plan)
    holes = len([lp for lp in plan.get("inner", []) if lp.get("shape") == "hole"]) if plan.get("outer") else len((m.get("step") or {}).get("holes", []))
    bends = len(plan.get("bends") or []) or len((m.get("step") or {}).get("bends", []))
    if not fs:
        raise ScsError("no flat size measured: give the flat DXF with --flat")
    if kind == "sheetmetal":
        return {"flat_x_mm": round(fs[0] * MM, 2), "flat_y_mm": round(fs[1] * MM, 2), "cut_length_mm": round((cl or 0) * MM, 1), "holes": holes, "bends": bends}
    net = (plan["outer"]["area"] - sum(lp["area"] for lp in plan["inner"]) + sum(lp["area"] for lp in plan["islands"])) if plan.get("outer") else fs[0] * fs[1]
    ins = sorted([fs[0], fs[1], t_in], reverse=True)
    cm = MM / 10
    return {"volume_cm3": round(net * t_in * cm ** 3, 3), "area_cm2": round((2 * net + (cl or 0) * t_in) * cm ** 2, 2),
            "size_x_mm": round(ins[0] * MM, 2), "size_y_mm": round(ins[1] * MM, 2), "size_z_mm": round(ins[2] * MM, 2)}


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_fab(args, data, m, flat, rep):
    mat = data.material(args.sku)
    finish = parse_finish(args.finish)
    kind, opts, unmapped = map_options(mat, finish, data)
    t_in = num(mat.get("thickness")) or 0
    out = Path(args.output) if args.output else Path(args.part).with_suffix("").with_suffix(".fab.json")
    base = out.parent.resolve()
    rel = lambda p: os.path.relpath(Path(p).resolve(), base)
    old = {}
    if out.is_file():
        try:
            old = json.loads(out.read_text())
        except ValueError:
            old = {}
    services = rep.services
    rows = rep.json()
    feats = features(kind, m, flat, t_in)
    if kind == "sheetmetal":
        hs = holes_of(flat or m)
        feats["countersinks"] = sum(1 for h in hs if any(near_d(h["d"], d) for d, _ in parse_pairs(args.countersink, num)))
    fab = {"kind": kind, "name": args.name or old.get("name") or Path(args.part).stem, "source": rel(args.part), "qty": args.qty or old.get("qty") or 1,
           "options": opts, "features": feats,
           "sendcutsend": {"sku": args.sku, "material": mat.get("name"), "thickness_in": t_in, "process": mat.get("cutting_process"),
                           "finish": {"type": finish[0], "value": finish[1]} if finish else None, "services": services,
                           "bend_angles": [float(a) for a in args.bend_angle], "hardware": list(args.hardware),
                           "taps": [{"d_in": d, "thread": t} for d, t in parse_pairs(args.tap)], "countersinks": [{"d_in": d, "major_in": mj} for d, mj in parse_pairs(args.countersink, num)],
                           "flat": rel(args.flat) if args.flat else None, "unmapped": unmapped,
                           "dfm": {"at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "pass": rows["pass"], "fail": rows["fail"], "warn": rows["warn"],
                                   "failed": [f"{x['rule']}: {x['text']}" for x in rows["rules"] if x["status"] == "fail"][:8],
                                   "source_sha256": sha256(args.part), "flat_sha256": sha256(args.flat) if args.flat else None,
                                   "catalog_generated_at": (data.catalog.get("_meta") or {}).get("generated_at"), "specs_generated_at": (data.specs.get("_meta") or {}).get("generated_at")}}}
    if old.get("order") is False:
        fab["order"] = False
    out.write_text(json.dumps(fab, indent=2) + "\n")
    return out, fab


# ---------------------------------------------------------------- the command line

def parse_finish(s):
    if not s:
        return None
    t, _, v = s.partition(":")
    return (t.strip(), v.strip() or None)


def parse_pairs(items, second=str):
    out = []
    for it in items or []:
        a, _, b = it.partition(":")
        d = thickness_in(a) if re.search(r"[a-z]", a, re.I) else float(a)
        out.append((d, second(b) if second is not str else b))
    return out


def from_fab(args):
    """`scs.py dfm part.fab.json`: the part, material and services come from its sendcutsend block."""
    fp = Path(args.part)
    o = json.loads(fp.read_text())
    s = o.get("sendcutsend") or {}
    if not s.get("sku"):
        raise ScsError(f"{fp} has no sendcutsend block with a sku (write it with `scs.py fab`)")
    base = fp.parent
    args.part = str(base / o["source"])
    args.sku = args.sku or s["sku"]
    if s.get("flat") and not args.flat:
        args.flat = str(base / s["flat"])
    args.service = args.service or [x for x in s.get("services", []) if x not in ("powder", "anodizing", "plating")]
    if s.get("finish") and not args.finish:
        args.finish = f"{s['finish']['type']}:{s['finish'].get('value') or ''}"
    args.bend_angle = args.bend_angle or [str(a) for a in s.get("bend_angles", [])]
    args.hardware = args.hardware or s.get("hardware", [])
    args.tap = args.tap or [f"{t['d_in']}:{t['thread']}" for t in s.get("taps", [])]
    args.countersink = args.countersink or [f"{c['d_in']}:{c.get('major_in') or ''}" for c in s.get("countersinks", [])]
    return args


def run_dfm(args, fetcher):
    if args.part.endswith(".fab.json"):
        args = from_fab(args)
    if not args.sku:
        raise ScsError("say which material: --sku SKU (find it with `scs.py materials`)")
    data = Data(fetcher)
    m = measure(args.part, args.bend_layer)
    flat = measure(args.flat, args.bend_layer) if args.flat else None
    rep = dfm(m, data, args.sku, services=args.service, finish=parse_finish(args.finish), bend_angles=[float(a) for a in args.bend_angle],
              hardware=args.hardware, taps=parse_pairs(args.tap), countersinks=parse_pairs(args.countersink, num), flat=flat)
    return data, m, flat, rep


def strip(m):
    """The model as JSON, loops shortened to what a reader needs."""
    def lp(x):
        o = {k: x[k] for k in ("shape", "d", "w", "l", "area") if k in x}
        if "c" in x:
            o["c"] = x["c"]
        o["bbox"] = bbox(x["pts"])
        return o
    out = {k: m[k] for k in ("source", "kind", "units", "units_known") if k in m}
    out["notes"] = m.get("notes")
    if m.get("outer"):
        out["outer"] = lp(m["outer"])
    out["inner"] = [lp(x) for x in m.get("inner", [])]
    out["islands"] = [lp(x) for x in m.get("islands", [])]
    out["separate_outlines"] = len(m.get("others", []))
    out["open_paths"] = [{"from": o["pts"][0], "to": o["pts"][-1]} for o in m.get("opens", [])]
    out["bends"] = m.get("bends", [])
    fs, how = flat_size(m)
    out["flat_size_in"], out["flat_how"], out["cut_length_in"] = fs, how, cut_length(m)
    if m.get("step"):
        out["step"] = {k: v for k, v in m["step"].items() if k != "flat_loops"}
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(prog="scs.py", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    f = sub.add_parser("fetch")
    f.add_argument("--refresh", action="store_true", help="revalidate even a copy younger than the TTL")
    sub.add_parser("status")
    sub.add_parser("guide")
    mt = sub.add_parser("materials")
    for k in ("q", "group", "category", "process", "service", "thickness"):
        mt.add_argument("--" + k)
    mt.add_argument("--json", action="store_true")
    one = sub.add_parser("material")
    one.add_argument("sku")
    fi = sub.add_parser("finishes")
    fi.add_argument("--type")
    hw = sub.add_parser("hardware")
    for k in ("q", "category", "sku"):
        hw.add_argument("--" + k)
    me = sub.add_parser("measure")
    me.add_argument("part")
    me.add_argument("--bend-layer", default=r"bend")
    for name in ("dfm", "fab"):
        d = sub.add_parser(name)
        d.add_argument("part")
        d.add_argument("--sku")
        d.add_argument("--flat", help="the flat pattern DXF of a bent STEP part")
        d.add_argument("--service", action="append", default=[])
        d.add_argument("--finish", help="TYPE:VALUE, e.g. powder:matte_black, anodizing:black")
        d.add_argument("--bend-angle", action="append", default=[])
        d.add_argument("--hardware", action="append", default=[])
        d.add_argument("--tap", action="append", default=[], help="HOLE_DIA_IN:THREAD, e.g. 0.159:10-32 or 3.3mm:M4x0.7")
        d.add_argument("--countersink", action="append", default=[], help="HOLE_DIA_IN:MAJOR_DIA_IN")
        d.add_argument("--bend-layer", default=r"bend")
        d.add_argument("--json", action="store_true")
        if name == "fab":
            d.add_argument("--name")
            d.add_argument("--qty", type=int)
            d.add_argument("-o", "--output")
    a = ap.parse_args(argv)
    fetcher = Fetcher()
    try:
        if a.cmd == "fetch":
            for n in SOURCES:
                fetcher.get(n, refresh=a.refresh)
                print(fetcher.line(n))
        elif a.cmd == "status":
            fetcher.offline = True
            for n in SOURCES:
                try:
                    s = fetcher.get(n)
                    print(f"{SOURCES[n][1]}: fetched {stamp(s['fetched_at'])} ({age_text(s['age'])} old), last checked {age_text(fetcher.now() - s['checked_at'])} ago, {s['path']}")
                except ScsError as e:
                    print(f"{SOURCES[n][1]}: no cached copy ({e})")
        elif a.cmd == "guide":
            print(fetcher.line("guide"), file=sys.stderr)
            sys.stdout.write(fetcher.get("guide")["data"].decode("utf-8"))
        elif a.cmd == "materials":
            data = Data(fetcher)
            rows = list(data.mats.values())
            q = (a.q or "").lower()
            if q:
                rows = [m for m in rows if q in json.dumps(m).lower()]
            for k in ("group", "category"):
                if getattr(a, k):
                    rows = [m for m in rows if getattr(a, k).lower() in (m.get(k) or "").lower()]
            if a.process:
                rows = [m for m in rows if a.process.lower() in (m.get("cutting_process") or "").lower()]
            if a.service:
                rows = [m for m in rows if a.service in (m.get("available_services") or [])]
            if a.thickness:
                t = thickness_in(a.thickness)
                rows = [m for m in rows if num(m.get("thickness")) is not None and abs(num(m["thickness"]) - t) <= max(0.004, 0.05 * t)]
            print(fetcher.line("catalog"), file=sys.stderr)
            if a.json:
                print(json.dumps(rows, indent=1))
            for m in rows if not a.json else []:
                t = num(m.get("thickness"))
                ts = f'{m.get("thickness")}" ({t * MM:.2f} mm)' if t else m.get("form", "")
                print(f"{m['sku']:<18} {m.get('name', ''):<34} {ts:<18} {m.get('cutting_process', ''):<12} {'OUT OF STOCK ' if m.get('out_of_stock') else ''}{','.join(m.get('available_services') or [])}")
        elif a.cmd == "material":
            data = Data(fetcher)
            print(fetcher.line("catalog"), file=sys.stderr)
            print(fetcher.line("specs"), file=sys.stderr)
            print(json.dumps({"catalog": data.material(a.sku), "specs": data.spec.get(a.sku)}, indent=1))
        elif a.cmd == "finishes":
            data = Data(fetcher)
            for x in data.finishes:
                if not a.type or x.get("finish_type") == a.type:
                    print(f"{x.get('finish_type'):<10} {x.get('value'):<24} {x.get('color', ''):<16} {x.get('code', '')}")
        elif a.cmd == "hardware":
            data = Data(fetcher)
            for x in data.hw.values():
                if a.sku and x["sku"] != a.sku:
                    continue
                if a.q and a.q.lower() not in json.dumps(x).lower():
                    continue
                if a.category and a.category.lower() not in (x.get("category") or "").lower():
                    continue
                if a.sku:
                    print(json.dumps(x, indent=1))
                else:
                    ms = x.get("manufacturing_specs") or {}
                    print(f"{x['sku']:<16} {x.get('title', ''):<44} hole Ø{x.get('hole_diameter')}\" edge≥{x.get('min_cl_to_edge_distance')}\" sheet {ms.get('min_sheet_thickness')}..{ms.get('max_sheet_thickness')}\"")
        elif a.cmd == "measure":
            print(json.dumps(strip(measure(a.part, a.bend_layer)), indent=1, default=list))
        elif a.cmd in ("dfm", "fab"):
            data, m, flat, rep = run_dfm(a, fetcher)
            if a.cmd == "fab":
                out, fab = write_fab(a, data, m, flat, rep)
                print(rep.text() if not a.json else json.dumps(rep.json(), indent=1))
                print(f"\nwrote {out}: kind {fab['kind']}, options {json.dumps(fab['options'])}, features {json.dumps(fab['features'])}" + (f"; no Backplane equivalent for: {', '.join(fab['sendcutsend']['unmapped'])}" if fab["sendcutsend"]["unmapped"] else ""))
            else:
                print(json.dumps(rep.json(), indent=1) if a.json else rep.text())
            return 1 if rep.failed() else 0
    except ScsError as e:
        print(f"scs.py: {e}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
