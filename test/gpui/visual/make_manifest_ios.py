#!/usr/bin/env python3
"""Build the iOS logo/cats manifests for check_logo_cats.py.

usage: make_manifest_ios.py LC_DIR WEB_DIR OUT_DIR
  LC_DIR   build/gpui/results/ios/lc   (ios-<dev>-<scheme>-<page>.png/.geom from scripts/gpui/ios-lc.sh)
  WEB_DIR  build/gpui/shots/logo-cats  (web-<profile>-<scheme>-<page>.png/.json from web-phone-shots.mjs)
Writes OUT_DIR/manifest-ios-<dev>-<scheme>.json (dev phone|ipad) and exits 1 if a logo or cat is not
found, or a size differs from the web's (the logo and cat sizes must match to the point).
The logo's rectangle is found in the screenshot by the mark's own colour (the toolbar hides its frame);
cats come from the app's BACKPLANE_GEOM log, paired with the web's by order down the page.
"""
import json, os, re, sys
from PIL import Image

lc, web, out = sys.argv[1:4]
SCALE = {"phone": 3, "ipad": 2}
PROFILE = {"phone": "phone", "ipad": "tablet"}
FG = {"dark": (0xe6, 0xe9, 0xde), "light": (0, 0, 0)}
svg = open(os.path.join(os.path.dirname(__file__), "../../../assets/icon/backplane.svg")).read()
tx, ty, sc = map(float, re.search(r"translate\(([\d.]+) ([\d.]+)\) scale\(([\d.]+)\)", svg).groups())
nums = [float(x) for x in re.findall(r"-?\d+\.\d+", re.search(r' d="([^"]+)"', svg).group(1))]
xs, ys = nums[0::2], nums[1::2]
# the mark's box inside the 256 tile, as fractions
MX0, MX1 = (tx + sc * min(xs)) / 256, (tx + sc * max(xs)) / 256
MY0, MY1 = (ty + sc * min(ys)) / 256, (ty + sc * max(ys)) / 256
bad = []


def find_logo(png, scheme, scale, band):
    """tile rect (px) found from the pixels in the mark's colour inside band (pt: x0,y0,x1,y1)"""
    im = Image.open(png).convert("RGB")
    x0, y0, x1, y1 = [int(v * scale) for v in band]
    fg, tol = FG[scheme], 16 if scheme == "dark" else 40
    px = im.load()
    pts = [(x, y) for y in range(y0, min(y1, im.height)) for x in range(x0, min(x1, im.width))
           if all(abs(px[x, y][i] - fg[i]) <= tol for i in range(3))]
    if len(pts) < 40:
        return None
    X0, X1 = min(p[0] for p in pts), max(p[0] for p in pts) + 1
    Y0, Y1 = min(p[1] for p in pts), max(p[1] for p in pts) + 1
    k = ((X1 - X0) / (MX1 - MX0) + (Y1 - Y0) / (MY1 - MY0)) / 2
    return [round(X0 - k * MX0, 1), round(Y0 - k * MY0, 1), round(k, 1), round(k, 1)]


CONN = {"dark": [(0x58, 0xc2, 0x8a), (0xf0, 0x6a, 0x6a)], "light": [(0x1c, 0x85, 0x50), (0xc9, 0x34, 0x34)]}


def find_logo_by_dot(png, scheme, scale, band, size_pt):
    """the header logo's tile: its connection square (7pt, ok or bad colour) sits 1pt past the tile's right
    edge and level with its bottom edge (style.css .brand-mark .conn), so the dot places the tile"""
    im = Image.open(png).convert("RGB"); px = im.load()
    x0, y0, x1, y1 = [int(v * scale) for v in band]
    pts = [(x, y) for y in range(y0, min(y1, im.height)) for x in range(x0, min(x1, im.width))
           if any(all(abs(px[x, y][i] - c[i]) <= 40 for i in range(3)) for c in CONN[scheme])]
    if len(pts) < 20:
        return None
    r, b = max(p[0] for p in pts) + 1, max(p[1] for p in pts) + 1
    k = size_pt * scale
    return [round(r - scale - k, 1), round(b - k, 1), k, k]


def logged_size(dev, scheme, page):
    """the logo's size in pt as the app drew it (BACKPLANE_GEOM `brand` lines)"""
    sizes = [float(m.group(1)) for m in (re.match(r"brand \S+ \S+ (\S+) \S+", l) for l in open(f"{lc}/ios-{dev}-{scheme}-{page}.geom")) if m]
    return sizes[-1] if sizes else None


def items_web(dev, scheme, page):
    return json.load(open(f"{web}/web-{PROFILE[dev]}-{scheme}-{page}.json"))


def geoms(dev, scheme, page):
    g = []
    for line in open(f"{lc}/ios-{dev}-{scheme}-{page}.geom"):
        m = re.match(r"(cat\w*:\S+) ([\d.-]+) ([\d.-]+) ([\d.-]+) ([\d.-]+)", line)
        if m:
            g.append((m.group(1), [float(m.group(i)) for i in range(2, 6)]))
    seen, uniq = set(), []
    for n, r in g:  # a cat can log twice (a list redraw): the last frame of each key wins
        seen.add(n)
    last = {}
    for n, r in g:
        last[n] = r
    return sorted(last.items(), key=lambda kv: (kv[1][1], kv[1][0]))


for dev in ("phone", "ipad"):
    scale = SCALE[dev]
    for scheme in ("dark", "light"):
        items = []
        rel = lambda p: os.path.relpath(p, out)
        # header logo: the list screen
        iosl = f"{lc}/ios-{dev}-{scheme}-list.png"
        webl = items_web(dev, scheme, "list")
        wl = [i for i in webl if i["kind"] == "logo"]
        band = (0, 40, 90, 130) if dev == "phone" else (0, 80, 90, 150)
        ls = logged_size(dev, scheme, "list")
        r = find_logo_by_dot(iosl, scheme, scale, band, ls) if ls else None
        if r is None or not wl:
            bad.append(f"{dev}/{scheme}: header logo not found (ios {r}, web {len(wl)})")
        else:
            if abs(ls * scale - wl[0]["rect"][2]) > 1.5:
                bad.append(f"{dev}/{scheme}: header logo is {ls}pt, the web draws {wl[0]['rect'][2]/scale:.1f}")
            items.append({"name": "logo-header", "kind": "logo",
                          "web": {"png": rel(f"{web}/web-{PROFILE[dev]}-{scheme}-list.png"), "rect": wl[0]["rect"]},
                          "native": {"png": rel(iosl), "rect": r}})
        # the pair screen's logo against the same web asset
        iosp = f"{lc}/ios-{dev}-{scheme}-pair.png"
        r = find_logo(iosp, scheme, scale, (0, 50, 130, 140) if dev == "phone" else (250, 55, 330, 110))
        if r is None or not wl:
            bad.append(f"{dev}/{scheme}: pair-screen logo not found")
        else:
            ps = logged_size(dev, scheme, "pair")  # the toolbar-free page logs its own size
            cx, cy = r[0] + r[2] / 2, r[1] + r[3] / 2
            if ps:  # the colour match shaves antialiased edge pixels: place the tile at the size the app logged
                r = [round(cx - ps * scale / 2, 1), round(cy - ps * scale / 2, 1), ps * scale, ps * scale]
            w48 = items_web(dev, scheme, "logo48")[0]["rect"]
            if abs(r[2] - w48[2] * 1.0) > 1.5:
                bad.append(f"{dev}/{scheme}: pair-screen logo is {r[2]/scale:.1f}pt, the 48 px reference is {w48[2]/scale:.1f}")
            items.append({"name": "logo-pair-screen", "kind": "logo",
                          "web": {"png": rel(f"{web}/web-{PROFILE[dev]}-{scheme}-logo48.png"), "rect": w48},
                          "native": {"png": rel(iosp), "rect": r}})
        # cats: the bots page, in order down the page; then the bot's own header
        wb = [i for i in items_web(dev, scheme, "bots") if i["kind"] == "cat"]
        ib = geoms(dev, scheme, "bots")
        ib = [(n, r) for n, r in ib if n.startswith("cat:") and n.endswith(":28")]
        if len(ib) != len(wb) or not wb:
            bad.append(f"{dev}/{scheme}: {len(ib)} cats on the bots page, the web has {len(wb)}")
        for k, ((n, r), w) in enumerate(zip(ib, wb)):
            if abs(r[2] * scale - w["rect"][2]) > 1.5:
                bad.append(f"{dev}/{scheme}: cat {k} is {r[2]}pt, the web draws {w['rect'][2]/scale:.1f}")
            items.append({"name": f"cat-list-{k}-{n.split(':')[1]}", "kind": "cat",
                          "web": {"png": rel(f"{web}/web-{PROFILE[dev]}-{scheme}-bots.png"), "rect": w["rect"]},
                          "native": {"png": rel(f"{lc}/ios-{dev}-{scheme}-bots.png"), "rect": [round(v * scale, 1) for v in r]}})
        wh = [i for i in items_web(dev, scheme, "bot") if i["kind"] == "cat" and not i["name"].endswith(":28")]
        ih = [(n, r) for n, r in geoms(dev, scheme, "bot") if n.startswith("cat:") and not n.endswith(":28") and r[0] >= 0 and r[1] >= 0]
        if len(wh) != 1 or len(ih) < 1:
            bad.append(f"{dev}/{scheme}: bot header cat: web {len(wh)}, ios {len(ih)}")
        else:
            n, r = ih[0]
            if abs(r[2] * scale - wh[0]["rect"][2]) > 1.5:
                bad.append(f"{dev}/{scheme}: header cat is {r[2]}pt, the web draws {wh[0]['rect'][2]/scale:.1f}")
            items.append({"name": "cat-header-Bub", "kind": "cat",
                          "web": {"png": rel(f"{web}/web-{PROFILE[dev]}-{scheme}-bot.png"), "rect": wh[0]["rect"]},
                          "native": {"png": rel(f"{lc}/ios-{dev}-{scheme}-bot.png"), "rect": [round(v * scale, 1) for v in r]}})
        # the look picker: the first six looks, drawn as each look's idle cat (40 px), paired by look index
        wlk = {}
        for i in items_web(dev, scheme, "look"):
            mm_ = re.match(r"cat:(\d+):idle:40:", i["name"] + ":")
            if i["kind"] == "cat" and mm_ and i["rect"][2] > 0:
                wlk.setdefault(int(mm_.group(1)), i)
        ilk = {}
        for n, r in geoms(dev, scheme, "look"):
            mm_ = re.match(r"catlook:(\d+):idle:40$", n)
            if mm_ and r[0] >= 0 and r[1] >= 0:
                ilk[int(mm_.group(1))] = r
        picks = [k for k in range(6) if k in wlk and k in ilk]
        if len(picks) < 6:
            bad.append(f"{dev}/{scheme}: look picker: {len(picks)} of the first 6 looks on both sides (web {sorted(wlk)[:8]}, ios {sorted(ilk)[:8]})")
        for k in picks:
            if abs(ilk[k][2] * scale - wlk[k]["rect"][2]) > 1.5:
                bad.append(f"{dev}/{scheme}: look {k} is {ilk[k][2]}pt, the web draws {wlk[k]['rect'][2]/scale:.1f}")
            items.append({"name": f"cat-look-{k}", "kind": "cat",
                          "web": {"png": rel(f"{web}/web-{PROFILE[dev]}-{scheme}-look.png"), "rect": wlk[k]["rect"]},
                          "native": {"png": rel(f"{lc}/ios-{dev}-{scheme}-look.png"), "rect": [round(v * scale, 1) for v in ilk[k]]}})
        # a room's posts: each carries its author's cat (a bot's, or 0:away for an unknown author), paired down the page
        wr = [i for i in items_web(dev, scheme, "room") if i["kind"] == "cat" and i["rect"][2] > 0]
        if dev == "ipad":  # the tablet web layout keeps the bots list beside the room: posts are to the right of it
            wr = [i for i in wr if i["rect"][0] > 300]
        ir = [(n, r) for n, r in geoms(dev, scheme, "room") if n.startswith("catpost:") and r[0] >= 0 and r[1] >= 0]
        if len(wr) != len(ir) or not wr:
            bad.append(f"{dev}/{scheme}: room posts: {len(ir)} cats on iOS, the web has {len(wr)}")
        for k, ((n, r), w) in enumerate(zip(ir, wr)):
            if abs(r[2] * scale - w["rect"][2]) > 1.5:
                bad.append(f"{dev}/{scheme}: room post cat {k} is {r[2]}pt, the web draws {w['rect'][2]/scale:.1f}")
            items.append({"name": f"cat-room-post-{k}", "kind": "cat",
                          "web": {"png": rel(f"{web}/web-{PROFILE[dev]}-{scheme}-room.png"), "rect": w["rect"]},
                          "native": {"png": rel(f"{lc}/ios-{dev}-{scheme}-room.png"), "rect": [round(v * scale, 1) for v in r]}})
        for it in items:  # whole pixels: check_logo_cats.py crops and resizes them
            for side in ("web", "native"):
                it[side]["rect"] = [int(round(v)) for v in it[side]["rect"]]
        m = {"platform": f"ios-{dev}", "scheme": scheme, "scale": scale, "items": items}
        json.dump(m, open(f"{out}/manifest-ios-{dev}-{scheme}.json", "w"), indent=1)
        print(f"manifest-ios-{dev}-{scheme}.json: {len(items)} items")
for b in bad:
    print("FAIL", b)
sys.exit(1 if bad else 0)
