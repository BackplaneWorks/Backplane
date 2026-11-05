#!/usr/bin/env python3
"""Colour check: every theme token of src/web/style.css must come out of GPUI with the same pixel value.

  check_colors.py SHOT.png LOG.txt SCHEME [--adapter NAME] [--selftest]

SHOT is a screenshot of `backplane-gpui --swatches --theme SCHEME`; LOG its stderr (GPUI_RECT lines name each swatch, GPUI_METRIC gpu names
the adapter). Expected values come from style.css parsed here, independently of theme.rs (the light block over :root).
Tolerance: +-1/255 per channel on flat swatches (a swapchain format or sRGB conversion error is tens off); translucent white/black over the page
background must equal the sRGB-space blend the web does, +-2/255 (linear-space blending would be 20+ off). One line `PASS|FAIL name reason`, exit 1 on any FAIL.
The adapter is printed, because the swapchain format may differ by adapter."""
import json, re, sys, os
from PIL import Image
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
FLAT_TOL, BLEND_TOL = 1, 2

def css_tokens(scheme):
    css = open(f"{ROOT}/src/web/style.css").read()
    def block(sel):
        body = css.split(sel)[1].split("}")[0]
        return {k: int(v[1:], 16) for k, v in re.findall(r"--([a-z-]+):\s*(#[0-9a-fA-F]{6});", body)}
    dark = block(":root {")
    return {**dark, **block("html.light, .app.light, .bub-move.light {")} if scheme == "light" else dark

def rects_from_log(text):
    out, adapter = {}, None
    for line in text.splitlines():
        if line.startswith("GPUI_RECT "):
            r = json.loads(line[10:]); out[r["name"]] = r
        elif line.startswith("GPUI_METRIC ") and '"event":"gpu"' in line:
            adapter = json.loads(line[12:]).get("device")
    return out, adapter

def centre(im, r, scale_hint=None):
    cx, cy = int(r["x"] + r["w"] / 2), int(r["y"] + r["h"] / 2)
    return im.getpixel((cx, cy))

def run(shot, log, scheme, adapter_label=None):
    im = Image.open(shot).convert("RGB")
    rects, adapter = rects_from_log(open(log).read())
    tokens = css_tokens(scheme)
    print(f"adapter: {adapter_label or adapter}  scheme: {scheme}  swatches: {sum(k.startswith('swatch:') for k in rects)}")
    bad = 0
    bg = tokens["bg"]; bgc = ((bg >> 16) & 255, (bg >> 8) & 255, bg & 255)
    for name, hexv in sorted(tokens.items()):
        r = rects.get("swatch:" + name)
        if not r: print(f"FAIL {name} swatch missing from the log"); bad += 1; continue
        want = ((hexv >> 16) & 255, (hexv >> 8) & 255, hexv & 255); got = centre(im, r)
        d = max(abs(a - b) for a, b in zip(want, got))
        if d > FLAT_TOL: print(f"FAIL {name} expected {want} got {got} (off by {d})"); bad += 1
        else: print(f"PASS {name} {got}")
    for label, c in (("white", 255), ("black", 0)):
        for alpha in (64, 128, 192):
            r = rects.get(f"blend:{label}:{alpha}")
            if not r: print(f"FAIL blend:{label}:{alpha} missing"); bad += 1; continue
            a = alpha / 255.0; want = tuple(round(c * a + b * (1 - a)) for b in bgc); got = centre(im, r)
            d = max(abs(x - y) for x, y in zip(want, got))
            if d > BLEND_TOL: print(f"FAIL blend:{label}:{alpha} expected {want} (sRGB blend) got {got} (off by {d})"); bad += 1
            else: print(f"PASS blend:{label}:{alpha} {got}")
    return bad

def selftest():
    import tempfile
    tokens = css_tokens("dark"); ok = True
    with tempfile.TemporaryDirectory() as t:
        lines, im = [], Image.new("RGB", (800, 400), ((tokens["bg"] >> 16) & 255, (tokens["bg"] >> 8) & 255, tokens["bg"] & 255))
        for i, (name, v) in enumerate(sorted(tokens.items())):
            x, y = 16 + (i % 8) * 76, 16 + (i // 8) * 52
            im.paste(((v >> 16) & 255, (v >> 8) & 255, v & 255), (x, y, x + 64, y + 40))
            lines.append("GPUI_RECT " + json.dumps({"name": "swatch:" + name, "x": x, "y": y, "w": 64, "h": 40, "scale": 1}))
        bgc = ((tokens["bg"] >> 16) & 255, (tokens["bg"] >> 8) & 255, tokens["bg"] & 255)
        for j, (label, c) in enumerate((("white", 255), ("black", 0))):
            for k, alpha in enumerate((64, 128, 192)):
                x, y = 16 + k * 76, 300 + j * 52; a = alpha / 255
                im.paste(tuple(round(c * a + b * (1 - a)) for b in bgc), (x, y, x + 64, y + 40))
                lines.append("GPUI_RECT " + json.dumps({"name": f"blend:{label}:{alpha}", "x": x, "y": y, "w": 64, "h": 40, "scale": 1}))
        log = os.path.join(t, "log.txt"); open(log, "w").write("\n".join(lines))
        good = os.path.join(t, "good.png"); im.save(good)
        import io, contextlib
        def quiet(shot):
            with contextlib.redirect_stdout(io.StringIO()): return run(shot, log, "dark")
        cases = [("good", good, 0)]
        # broken inputs: one token off by 6, channels rotated, linear-blend white 128 (188 over black-ish bg)
        b1 = im.copy(); x, y = 16, 16; px = b1.getpixel((x + 5, y + 5)); b1.paste((px[0] + 6, px[1], px[2]), (x, y, x + 64, y + 40)); p1 = os.path.join(t, "b1.png"); b1.save(p1); cases.append(("one token off by 6", p1, 1))
        b2 = Image.merge("RGB", tuple(im.split()[i] for i in (2, 0, 1))); p2 = os.path.join(t, "b2.png"); b2.save(p2); cases.append(("channels rotated", p2, 1))
        b3 = im.copy(); b3.paste((188, 188, 188), (16 + 76, 300, 16 + 76 + 64, 340)); p3 = os.path.join(t, "b3.png"); b3.save(p3); cases.append(("linear-space blend of white 128", p3, 1))
        for name, shot, must_fail in cases:
            bad = quiet(shot); good_case = (bad > 0) == bool(must_fail); ok &= good_case
            print(("selftest ok   " if good_case else "selftest WRONG ") + f"{name}: {'FAIL as required' if bad else 'pass'}" + (f" ({bad} bad)" if bad else ""))
    return 0 if ok else 1

if __name__ == "__main__":
    a = sys.argv[1:]
    if "--selftest" in a: sys.exit(selftest())
    label = a[a.index("--adapter") + 1] if "--adapter" in a else None
    pos = [x for x in a if not x.startswith("--") and x != label]
    if len(pos) < 3: print(__doc__); sys.exit(2)
    sys.exit(1 if run(pos[0], pos[1], pos[2], label) else 0)
