#!/usr/bin/env python3
"""Shoot the web and GPUI at 1440x900 (scale 1 and 2, dark and light) against the seeded stand-in hub, write the
manifests the checker reads and side-by-side images, all under build/gpui/shots/logo-cats/.
  run_logo_cats.py [dark|light ...]      needs: hub on 127.0.0.1:3821 with bots (seed-bots.mjs), Xvfb :61 (1x) and :71 (2x, 2880x1800, Xft.dpi 192) with an i3 each."""
import json, os, re, subprocess, sys
from PIL import Image, ImageDraw
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
OUT = f"{ROOT}/build/gpui/shots/logo-cats"; HOME = f"{ROOT}/build/gpui/desktop-hub/home"; os.makedirs(OUT, exist_ok=True)
token = open(f"{HOME}/token").read().strip()
def sh(cmd, **kw): return subprocess.run(cmd, shell=True, check=True, capture_output=True, text=True, **kw).stdout
for scheme in (sys.argv[1:] or ["dark", "light"]):
    sh(f"bun {ROOT}/test/gpui/set-setting.mjs {HOME} 127.0.0.1:3821 ui.theme {scheme}")
    for scale in (1, 2):
        sh(f"systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 timeout 150 bun {ROOT}/test/gpui/visual/web-logo-cats.mjs http://127.0.0.1:3821 {token} {OUT} {scheme} {scale}")
        web_rects = json.load(open(f"{OUT}/web-{scheme}-{scale}x.rects.json"))
        disp = ":61" if scale == 1 else ":71"; name = f"gpui-{scheme}-{scale}x"
        sh(f"DISP={disp} {ROOT}/scripts/gpui/desktop-shot.sh {name} shot-thread.json 5")
        os.replace(f"{ROOT}/build/gpui/shots/desktop/{name}.png", f"{OUT}/{name}.png")
        native = {}
        for line in open(f"{ROOT}/build/gpui/results/desktop/{name}.log"):
            if line.startswith("GPUI_RECT "):
                r = json.loads(line[10:]); native[r["name"]] = [round(r["x"]), round(r["y"]), round(r["w"]), round(r["h"])]
        items = [{"name": n, "kind": "logo" if n == "logo" else "cat", "web": {"png": f"web-{scheme}-{scale}x.png", "rect": web_rects[n]},
                  "native": {"png": f"{name}.png", "rect": native[n]}} for n in web_rects if n in native]
        json.dump({"platform": "desktop-linux", "scheme": scheme, "scale": scale, "items": items}, open(f"{OUT}/manifest-desktop-{scheme}-{scale}x.json", "w"), indent=1)
        # side by side: web | gpui per item, nearest-neighbour enlarged so pixels show
        z = 8 if scale == 1 else 4; rows = []
        for it in items:
            a = Image.open(f"{OUT}/{it['web']['png']}").convert("RGB").crop((lambda r: (r[0], r[1], r[0] + r[2], r[1] + r[3]))(it["web"]["rect"]))
            b = Image.open(f"{OUT}/{it['native']['png']}").convert("RGB").crop((lambda r: (r[0], r[1], r[0] + r[2], r[1] + r[3]))(it["native"]["rect"]))
            a, b = a.resize((a.width * z, a.height * z), Image.NEAREST), b.resize((b.width * z, b.height * z), Image.NEAREST); rows.append((it["name"], a, b))
        W = 2 * max(max(a.width, b.width) for _, a, b in rows) + 260; H = sum(max(a.height, b.height) + 10 for _, a, b in rows) + 30
        bg = (250, 250, 248) if scheme == "light" else (13, 15, 18); img = Image.new("RGB", (W, H), bg); d = ImageDraw.Draw(img)
        d.text((10, 8), f"{scheme} {scale}x   left: web   right: GPUI", fill=(128, 128, 128)); y = 30; cw = (W - 260) // 2
        for n, a, b in rows:
            d.text((10, y + 4), n, fill=(128, 128, 128)); img.paste(a, (130, y)); img.paste(b, (130 + cw, y)); y += max(a.height, b.height) + 10
        img.save(f"{OUT}/side-{scheme}-{scale}x.png"); print("wrote", f"side-{scheme}-{scale}x.png", len(items), "items")
