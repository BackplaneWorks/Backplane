#!/usr/bin/env python3
"""iOS font check: JetBrains Mono (SIL OFL) draws the mono text, ligatures off, CJK falls back to the system font.

  check_fonts_ios.py SHOT.png FONT_DIR SCHEME SCALE [--selftest]

SHOT is the simulator screenshot after BACKPLANE_ACTS "@fonts" (FontsProbe in Mono.swift: five lines at x=16, y=120+24i points, 13 pt,
line 4 bold; the same lines as the desktop's `--fonts`), SCALE the screen's pixels per point (3 on the iPhone, 2 on the iPad).
Same rules as test/gpui/visual/check_fonts.py (desktop, owner `desktop`), one line each, `PASS|FAIL name reason`, exit 1 on any FAIL:
  files   : the bundled files equal the repo's assets/fonts/jetbrains-mono (sha256 SHA256SUMS), OFL.txt and SOURCE.txt present
  line 0,1,4 : the ink box is within 4% of an independent FreeType render (Pillow, the same TTF, 13 pt x SCALE) in width and height +-2 px x SCALE,
               and the luminance of both (aligned on the ink box) correlates >= 0.55; line 1 holds ligature candidates (=> != -> ==): with ligatures on
               the line is narrower and the correlation falls
  line 3  : the CJK line is not blank (the system font fell back)"""
import os, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_fonts as C

Y0 = 120  # FontsProbe starts below the status bar / Dynamic Island

def band(im, i, s):
    return im.crop((0, int((Y0 + 24 * i) * s), int(390 * s), int((Y0 + 24 * i + 24) * s)))

def ref_ink(text, ttf, s, scheme):
    f = ImageFont.truetype(ttf, int(round(13 * s)))
    im = Image.new("RGB", (int(900 * s), int(24 * s)), C.BG[scheme]); dr = ImageDraw.Draw(im)
    try:  # the app turns ligatures and contextual alternates off: so does the reference (needs Pillow's raqm shaper)
        dr.text((0, int(5 * s)), text, font=f, fill=C.FG[scheme], features=["-liga", "-calt"])
    except (KeyError, ValueError, OSError):
        dr.text((0, int(5 * s)), text, font=f, fill=C.FG[scheme])
    l = C.lum(im); b = C.ink_box(l, C.lum(Image.new("RGB", (1, 1), C.BG[scheme]))[0, 0]); return l, b

def run(shot, fontdir, scheme, s):
    out = [(n, r) for n, r in C.files_check(fontdir)]
    im = Image.open(shot).convert("RGB")
    bg = C.lum(Image.new("RGB", (1, 1), C.BG[scheme]))[0, 0]
    for i in (0, 1, 4):
        ttf = os.path.join(fontdir, "JetBrainsMono-Bold.ttf" if i == 4 else "JetBrainsMono-Regular.ttf")
        got = C.lum(band(im, i, s)); gb = C.ink_box(got, bg)
        rl, rb = ref_ink(C.LINES[i], ttf, s, scheme)
        if gb is None: out.append((f"line {i}", "blank")); continue
        gw, gh, rw, rh = gb[2] - gb[0], gb[3] - gb[1], rb[2] - rb[0], rb[3] - rb[1]
        if abs(gw - rw) > 0.04 * rw or abs(gh - rh) > 2 * s:
            out.append((f"line {i}", f"ink box {gw}x{gh} vs the reference {rw}x{rh}")); continue
        a = got[gb[1]:gb[3], gb[0]:gb[2]]; b = rl[rb[1]:rb[3], rb[0]:rb[2]]
        h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
        c = float(np.corrcoef(a[:h, :w].ravel(), b[:h, :w].ravel())[0, 1])
        out.append((f"line {i}", "" if c >= 0.55 else f"correlation {c:.2f} < 0.55 (a different face, or ligatures on)"))
    cj = C.ink_box(C.lum(band(im, 3, s)), bg)
    out.append(("line 3 (CJK)", "" if cj and cj[2] - cj[0] > 40 * s / 3 else "blank: no fallback glyphs"))
    return out

def main():
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    shot, fontdir, scheme, s = a[0], a[1], a[2], float(a[3])
    if "--selftest" in sys.argv:
        # a different face (DejaVu Sans Mono) painted over line 0 must FAIL line 0
        im = Image.open(shot).convert("RGB"); d = ImageDraw.Draw(im)
        dv = next((p for p in ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",) if os.path.exists(p)), None)
        if not dv: print("selftest skipped: no DejaVu"); return 0
        x0, y0 = 0, int(Y0 * s); d.rectangle([x0, y0, int(390 * s), int((Y0 + 24) * s)], fill=C.BG[scheme])
        d.text((int(Y0 * s), y0 + int(5 * s)), C.LINES[0], font=ImageFont.truetype(dv, int(round(13 * s))), fill=C.FG[scheme])
        p = "/tmp/_fonts_selftest.png"; im.save(p)
        r = dict(run(p, fontdir, scheme, s)); bad = r.get("line 0", "")
        print(("selftest ok   line 0 swapped face: FAIL as required: " + bad) if bad else "selftest FAILED: the swapped face passed")
        return 0 if bad else 1
    res = run(shot, fontdir, scheme, s); bad = 0
    for n, r in res:
        print(("FAIL " if r else "PASS ") + n + (" " + r if r else " ok")); bad += bool(r)
    return 1 if bad else 0

sys.exit(main())
