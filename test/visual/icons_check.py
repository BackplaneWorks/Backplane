#!/usr/bin/env python3
"""Icon-presence check for `backplane-gpui --gallery`: every icon the web shows must be drawn, non-blank, sharp and equal to an
independent render of the same Lucide path data.
  icons_check.py SHOT.png LOG.txt ASSETS_DIR SCHEME OUTDIR
Writes OUTDIR/icons-ref.png and
OUTDIR/manifest-icons.json (form B for check_logo_cats.py). The caller then runs check_logo_cats.py on it (and --selftest)."""
import json, os, sys, io
from PIL import Image
shot, log, assets, scheme, out = sys.argv[1:6]
tokens = {"dark": {"bg": 0x0d0f12, "text": 0xd9dde4}, "light": {"bg": 0xfafaf8, "text": 0x1b1d21}}[scheme]
WEB = ["chevron-down", "pin", "circle-check", "archive", "trash", "git-fork", "message-square", "terminal", "search", "circuit-board", "cpu", "box",
       "layout-panel-left", "layers", "folder", "diff", "globe", "chevron-right", "chevron-left", "settings", "plus", "user", "square-pen", "gauge", "paperclip", "mic"]
icons = json.load(open(os.path.join(assets, "icons.json")))
rects = {}
for line in open(log):
    if line.startswith("GPUI_RECT "):
        r = json.loads(line[10:])
        if r["name"].startswith("icon:"): rects[r["name"][5:]] = r
im = Image.open(shot).convert("RGB")
bg = ((tokens["bg"] >> 16) & 255, (tokens["bg"] >> 8) & 255, tokens["bg"] & 255)
ref = Image.new("RGB", im.size, bg)
try:
    import cairosvg
except ImportError:
    cairosvg = None
items, missing = [], []
fg = "#%06x" % tokens["text"]
for name in WEB:
    r = rects.get(name)
    if name not in icons or not r: missing.append(name); continue
    x, y, w, h = round(r["x"]), round(r["y"]), round(r["w"]), round(r["h"])
    if cairosvg:
        paths = "".join('<path d="%s" %s/>' % (l["d"], ('fill="%s" stroke="none" fill-rule="evenodd"' % fg) if l["fill"] else "") for l in icons[name])
        svg = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="%d" height="%d" fill="none" stroke="%s" stroke-width="2" '
               'stroke-linecap="round" stroke-linejoin="round">%s</svg>') % (w, h, fg, paths)
        png = Image.open(io.BytesIO(cairosvg.svg2png(bytestring=svg.encode()))).convert("RGBA")
        ref.paste(png, (x, y), png)
    items.append({"name": "icon:" + name, "kind": "cat", "max_diff": 0.2, "web": {"png": "icons-ref.png", "rect": [x, y, w, h]}, "native": {"png": os.path.relpath(shot, out), "rect": [x, y, w, h]}})
ref.save(os.path.join(out, "icons-ref.png"))
json.dump({"platform": "desktop-linux", "scheme": scheme, "scale": 1, "items": items}, open(os.path.join(out, "manifest-icons.json"), "w"), indent=1)
print("icons in the gallery:", len(items), "missing:", missing, "reference renderer:", "cairosvg" if cairosvg else "NONE")
sys.exit(1 if missing or not cairosvg else 0)
