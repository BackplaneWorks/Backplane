#!/usr/bin/env python3
"""Side-by-side images, web (left) vs native (right), of every item of a manifest (form B), upscaled 4x.
usage: side_by_side.py MANIFEST.json OUT.png"""
import json, os, sys
from PIL import Image, ImageDraw
m = json.load(open(sys.argv[1])); base = os.path.dirname(os.path.abspath(sys.argv[1]))
rows = []
for it in m["items"]:
    def crop(side):
        x, y, w, h = [int(round(v)) for v in it[side]["rect"]]
        pad = max(6, w // 6)
        im = Image.open(os.path.join(base, it[side]["png"])).convert("RGB")
        c = im.crop((max(0, x - pad), max(0, y - pad), x + w + pad, y + h + pad))
        s = 4 if w <= 100 else 2
        return c.resize((c.width * s, c.height * s), Image.NEAREST)
    rows.append((it["name"], crop("web"), crop("native")))
H = sum(max(a.height, b.height) + 22 for _, a, b in rows)
W = max(a.width + b.width + 30 for _, a, b in rows)
out = Image.new("RGB", (W, H), (128, 128, 128)); d = ImageDraw.Draw(out); y = 0
for name, a, b in rows:
    d.text((4, y + 4), f"{name}   web (left) vs {m['platform']} {m['scheme']} x{m['scale']} (right)", fill=(255, 255, 0))
    out.paste(a, (0, y + 20)); out.paste(b, (a.width + 30, y + 20)); y += max(a.height, b.height) + 22
out.save(sys.argv[2])
