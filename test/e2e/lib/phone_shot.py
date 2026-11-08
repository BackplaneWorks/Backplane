#!/usr/bin/env python3
"""phone_shot.py A.png [B.png]: what a viewer page on the phone shows, from its screenshot (the viewer area of the 2424x1080 landscape screen).
Prints one JSON line: bright (pixels clearly lighter than the page background), green and blue (their share of the bright pixels: the board's 3D model is green, the
board's layers blue, the mechanical part grey), and, given B.png taken without any touch in between, still (the two are identical: nothing is animating, a spinner or a
loading card that redraws would make them differ)."""
import sys, json
from PIL import Image, ImageChops
def view(p): return Image.open(p).convert('RGB').crop((0, 330, 2424, 1000))
a = view(sys.argv[1]); px = a.getdata(); g = a.convert('L').getdata()
bg = sorted(g)[len(g) // 2]
bright = [p for p, l in zip(px, g) if l > bg + 40]
n = max(1, len(bright))
out = {"bright": len(bright), "green": round(sum(1 for r, gg, b in bright if gg > r + 15 and gg > b + 10) / n, 3), "blue": round(sum(1 for r, gg, b in bright if b > r + 25 and b > gg + 10) / n, 3)}
if len(sys.argv) > 2: out["still"] = ImageChops.difference(a, view(sys.argv[2])).getbbox() is None
print(json.dumps(out))
