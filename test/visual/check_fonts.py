#!/usr/bin/env python3
"""Font check: JetBrains Mono (SIL OFL) is bundled, loaded by GPUI and what draws the mono text.

  check_fonts.py SHOT.png LOG.txt FONT_DIR SCHEME [--out SIDE.png] [--selftest]

SHOT is `backplane-gpui --fonts` at 1x (line i at x=16, y=16+24i, 24 px high, 13 px mono; line 4 bold); LOG its stderr.
Checks (one line each, `PASS|FAIL name reason`, exit 1 on any FAIL):
  files    : JetBrainsMono-Regular.ttf and -Bold.ttf exist, each < 1 MB, sha256 equal to SHA256SUMS; OFL.txt holds the licence and SOURCE.txt the origin
  loaded   : the log has GPUI_LOG {"font":"JetBrains Mono","loaded":true}
  line N   : the ink box of the line is within 4% of an independent FreeType render (Pillow, same TTF, 13 px) in width and height +-2 px, and the
             luminance of both (aligned on the ink box) correlates >= 0.55 (a different face or ligatures turned on fall below)
  cjk      : the CJK line is not blank (the system font fell back), and symbols/box-drawing are drawn (non-blank, ink within 6% of the reference width)"""
import hashlib, json, os, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
LINES = ["The quick brown fox 0123456789 {}[]()<>", "=> != -> == <= >= :: ... |> ++ www",
         "┌─┬─┐│ ├─┼─┤ └─┴─┘ → ← ✓ ⌘", "日本語 中文 fallback", "Bold: The quick brown fox"]
BG = {"dark": (0x0d, 0x0f, 0x12), "light": (0xfa, 0xfa, 0xf8)}; FG = {"dark": (0xd9, 0xdd, 0xe4), "light": (0x1b, 0x1d, 0x21)}
SIZE = 13

def lum(im): a = np.asarray(im.convert("RGB"), dtype=float); return a[..., 0] * .299 + a[..., 1] * .587 + a[..., 2] * .114
def ink_box(l, bg_l):
    m = np.abs(l - bg_l) > 24
    if not m.any(): return None
    ys, xs = np.where(m); return xs.min(), ys.min(), xs.max() + 1, ys.max() + 1
def draw(d, xy, text, font, fill):
    try: d.text(xy, text, font=font, fill=fill, features=["-calt", "-liga", "-clig"])
    except (KeyError, ValueError, OSError): d.text(xy, text, font=font, fill=fill)
def reference(text, font, scheme):
    im = Image.new("RGB", (900, 24), BG[scheme]); draw(ImageDraw.Draw(im), (0, 5), text, font, FG[scheme]); return im  # ligatures off, as in the app

def files_check(d):
    out = []
    sums = {l.split()[1]: l.split()[0] for l in open(os.path.join(d, "SHA256SUMS")).read().splitlines() if l.strip()} if os.path.exists(os.path.join(d, "SHA256SUMS")) else {}
    for f in ("JetBrainsMono-Regular.ttf", "JetBrainsMono-Bold.ttf"):
        p = os.path.join(d, f)
        if not os.path.exists(p): out.append((f"file {f}", "missing")); continue
        if os.path.getsize(p) >= 1 << 20: out.append((f"file {f}", "over 1 MB")); continue
        h = hashlib.sha256(open(p, "rb").read()).hexdigest()
        out.append((f"file {f}", "" if sums.get(f) == h else f"sha256 {h[:12]} differs from SHA256SUMS"))
    ofl = os.path.join(d, "OFL.txt")
    out.append(("file OFL.txt", "" if os.path.exists(ofl) and "SIL OPEN FONT LICENSE" in open(ofl, errors="ignore").read().upper() else "licence missing"))
    out.append(("file SOURCE.txt", "" if os.path.exists(os.path.join(d, "SOURCE.txt")) else "missing"))
    return out

def run(shot, log, fontdir, scheme, side=None):
    res = files_check(fontdir)
    logtext = open(log).read() if os.path.exists(log) else ""
    def font_loaded(text):
        for line in text.splitlines():
            if line.startswith("GPUI_LOG "):
                try: j = json.loads(line[9:])
                except ValueError: continue
                if j.get("font") == "JetBrains Mono" and j.get("loaded") is True: return True
        return False
    res.append(("loaded", "" if font_loaded(logtext) else "GPUI_LOG font loaded:true not in the log"))
    im = Image.open(shot).convert("RGB"); bgl = lum(Image.new("RGB", (1, 1), BG[scheme]))[0, 0]
    reg = ImageFont.truetype(os.path.join(fontdir, "JetBrainsMono-Regular.ttf"), SIZE); bold = ImageFont.truetype(os.path.join(fontdir, "JetBrainsMono-Bold.ttf"), SIZE)
    rows = []
    for i, text in enumerate(LINES):
        nat = im.crop((16, 16 + 24 * i, 16 + 900, 16 + 24 * i + 24)); ref = reference(text, bold if i == 4 else reg, scheme)
        nb, rb = ink_box(lum(nat), bgl), ink_box(lum(ref), bgl)
        name = f"line {i}"
        if nb is None: res.append((name, "blank")); rows.append((nat, ref)); continue
        rows.append((nat, ref))
        if i == 3:  # CJK: the system fallback drew something wide enough for the CJK cells
            res.append(("cjk", "" if nb[2] - nb[0] >= 7.8 * 8 else f"ink width {nb[2]-nb[0]}px, too narrow for the fallback glyphs")); continue
        nw, nh, rw, rh = nb[2] - nb[0], nb[3] - nb[1], rb[2] - rb[0], rb[3] - rb[1]
        if abs(nw - rw) > max(0.04 * rw, 2) or abs(nh - rh) > 2: res.append((name, f"ink box {nw}x{nh} vs reference {rw}x{rh}")); continue
        a = lum(nat.crop(nb)); b = lum(ref.crop(rb).resize((nw, nh), Image.LANCZOS)); r = float(np.corrcoef(a.ravel(), b.ravel())[0, 1])
        res.append((name, "" if r >= 0.55 else f"correlation {r:.2f} < 0.55 (a different face, or ligatures on)"))
    if side:
        W = 900; img = Image.new("RGB", (W, 10 + 2 * 26 * len(rows)), BG[scheme]); y = 5
        for nat, ref in rows: img.paste(nat, (0, y)); img.paste(ref, (0, y + 26)); y += 52
        img.resize((W * 2, img.height * 2), Image.NEAREST).save(side)
    bad = 0
    for n, r in res: print(("FAIL " if r else "PASS ") + n + (" " + r if r else " ok")); bad += bool(r)
    return bad

def selftest(fontdir):
    import tempfile
    ok = True
    reg = ImageFont.truetype(os.path.join(fontdir, "JetBrainsMono-Regular.ttf"), SIZE); bold = ImageFont.truetype(os.path.join(fontdir, "JetBrainsMono-Bold.ttf"), SIZE)
    def sheet(fn):
        im = Image.new("RGB", (1000, 160), BG["dark"]); d = ImageDraw.Draw(im)
        for i, t in enumerate(LINES): fn(d, i, t)
        return im
    good = sheet(lambda d, i, t: draw(d, (16, 16 + 24 * i + 5), t, bold if i == 4 else reg, FG["dark"]))
    good = good  # the CJK line needs wide glyphs: draw ideographs as blocks like a fallback font would
    gd = ImageDraw.Draw(good); gd.rectangle((16, 16 + 72 + 5, 16 + 7.8 * 8 + 8, 16 + 72 + 18), outline=FG["dark"])
    wrong = sheet(lambda d, i, t: d.text((16, 16 + 24 * i + 5), t, font=ImageFont.load_default(), fill=FG["dark"]))
    blank = Image.new("RGB", (1000, 160), BG["dark"])
    with tempfile.TemporaryDirectory() as t:
        log_ok = os.path.join(t, "ok.log"); open(log_ok, "w").write('GPUI_LOG {"font":"JetBrains Mono","loaded":true}\n')
        log_bad = os.path.join(t, "bad.log"); open(log_bad, "w").write("")
        import io, contextlib
        def quiet(im, log):
            p = os.path.join(t, "s.png"); im.save(p)
            with contextlib.redirect_stdout(io.StringIO()): return run(p, log, fontdir, "dark")
        for name, im, log, must_fail in [("pillow render of the bundled font", good, log_ok, False), ("a different face", wrong, log_ok, True), ("blank screenshot", blank, log_ok, True), ("font not loaded in the log", good, log_bad, True)]:
            bad = quiet(im, log); g = (bad > 0) == must_fail; ok &= g
            print(("selftest ok   " if g else "selftest WRONG ") + f"{name}: {'FAIL as required' if bad else 'pass'}" + (f" ({bad} bad)" if bad else ""))
    return 0 if ok else 1

if __name__ == "__main__":
    a = sys.argv[1:]; here = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
    default = os.path.join(here, "assets/fonts/jetbrains-mono")
    if "--selftest" in a: sys.exit(selftest(next((x for x in a if os.path.isdir(x)), default)))
    side = a[a.index("--out") + 1] if "--out" in a else None
    pos = [x for x in a if not x.startswith("--") and x != side]
    if len(pos) < 4: print(__doc__); sys.exit(2)
    sys.exit(1 if run(pos[0], pos[1], pos[2], pos[3], side) else 0)
