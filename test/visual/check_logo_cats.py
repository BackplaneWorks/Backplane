#!/usr/bin/env python3
"""Visual check: the logo and every cat are present, not blank, right-coloured and sharp.

  check_logo_cats.py MANIFEST.json [--out REPORT.json] [--selftest]
  check_logo_cats.py --selftest

One line per item: `PASS|FAIL <id> <reason>`; exit 1 on any FAIL. Two manifest forms, told apart per item:

 A (android): {"items":[{"id","kind":"logo|cat","theme":"dark|light","image","box":[x,y,w,h],
                         ["reference": web crop png], ["mood"]}]}
 B (desktop/ios): {"platform","scheme":"dark|light","scale":1|2|3,
                   "items":[{"name","kind","web":{"png","rect"},"native":{"png","rect"}}]}

Paths resolve against the manifest's folder. Thresholds (all stated here, none hidden):
  blank      : < 3% of the region's pixels differ by > 24/255 (max channel) from its dominant colour,
               or luminance stdev <= 4                                              -> FAIL blank
  logo colour: background and glyph each >= 4% of pixels within tolerance 28 of
               dark #111410/#e6e9de or light #ffffff/#000000 (favicon-light.svg, commit 80cc30e499);
               a dark logo on a light theme (or the reverse) fails                    -> FAIL wrong colours
  cat        : >= 2 distinct non-background colour bins (32 levels/channel, each >= 1% of the
               non-background pixels) and a non-background dominant colour that is > 24 from the background -> FAIL flat
  reference  : form A too: mean RGB of the non-background pixels within 24/255 of the reference's (a recoloured or
               channel-rotated cat fails); both regions resized to 64x64: mean abs diff (0..1) <= 0.12 (logo) / 0.18 (cat: animation phase) -> FAIL differs
  form B only: mean RGB of non-background pixels within 24/255 of the web's; native Laplacian variance
               >= 0.5x the web's (a blurred raster fails)                             -> FAIL colour / blurry
"""
import json, sys, os, tempfile, copy
import numpy as np
from PIL import Image, ImageFilter

TOL_BLANK, MIN_FRAC, MIN_STD = 24, 0.03, 4.0
LOGO = {"dark": ((0x11, 0x14, 0x10), (0xe6, 0xe9, 0xde)), "light": ((0xff, 0xff, 0xff), (0x00, 0x00, 0x00))}
LOGO_TOL, LOGO_MIN = 28, 0.04
MAX_DIFF = {"logo": 0.12, "cat": 0.18}

def load(path, rect=None):
    im = Image.open(path).convert("RGB")
    if rect:
        x, y, w, h = rect
        if w <= 0 or h <= 0 or x < 0 or y < 0 or x + w > im.width or y + h > im.height:
            raise ValueError(f"rect {rect} outside {im.width}x{im.height}")
        im = im.crop((x, y, x + w, y + h))
    return im

def arr(im): return np.asarray(im, dtype=np.int32)
def dominant(a):
    q = (a // 4).reshape(-1, 3)
    keys, counts = np.unique(q[:, 0] * 4096 + q[:, 1] * 64 + q[:, 2], return_counts=True)
    k = int(keys[counts.argmax()])
    return np.array([(k // 4096) * 4 + 2, ((k // 64) % 64) * 4 + 2, (k % 64) * 4 + 2])
def lum(a): return a[..., 0] * 0.299 + a[..., 1] * 0.587 + a[..., 2] * 0.114
def foreground(a, bg): return np.abs(a - bg).max(axis=-1) > TOL_BLANK
def laplacian_var(a):
    l = lum(a)
    if min(l.shape) < 3: return 0.0
    lap = -4 * l[1:-1, 1:-1] + l[:-2, 1:-1] + l[2:, 1:-1] + l[1:-1, :-2] + l[1:-1, 2:]
    return float(lap.var())
def small(im): return arr(im.resize((64, 64), Image.LANCZOS)) / 255.0
def close(a, c, tol): return (np.abs(a - np.array(c)).max(axis=-1) <= tol)

def check_region(im, kind, theme):
    """Checks that need only the native region. Returns a reason string on failure, else ''."""
    a = arr(im); bg = dominant(a); fg = foreground(a, bg)
    if fg.mean() < MIN_FRAC or lum(a).std() <= MIN_STD:
        return f"blank ({fg.mean()*100:.1f}% foreground, stdev {lum(a).std():.1f})"
    if kind == "logo":
        want = LOGO[theme]; other = LOGO["light" if theme == "dark" else "dark"]
        b, g = close(a, want[0], LOGO_TOL).mean(), close(a, want[1], LOGO_TOL).mean()
        if b < LOGO_MIN or g < LOGO_MIN:
            ob, og = close(a, other[0], LOGO_TOL).mean(), close(a, other[1], LOGO_TOL).mean()
            hint = " (the other theme's logo)" if ob >= LOGO_MIN and og >= LOGO_MIN else ""
            return f"wrong colours for {theme}: background {b*100:.1f}% glyph {g*100:.1f}%{hint}"
    else:
        px = a[fg]
        if len(px) == 0: return "flat"
        bins = (px // 8).astype(int); key = bins[:, 0] * 1024 + bins[:, 1] * 32 + bins[:, 2]
        _, counts = np.unique(key, return_counts=True)
        if (counts >= 0.01 * len(px)).sum() < 2: return "flat: fewer than 2 distinct colours on the cat"
    return ""

def mean_rgb(a):
    fg = foreground(a, dominant(a))
    return a[fg].mean(axis=0) if fg.any() else np.zeros(3)

def run_item(item, base, default_scheme="dark"):
    """-> (id, reason or ''); never raises."""
    iid = item.get("id") or item.get("name") or "?"
    try:
        kind = item["kind"]
        p = lambda s: s if os.path.isabs(s) else os.path.join(base, s)
        if "native" in item:  # form B
            theme = item.get("theme", default_scheme)
            nat = load(p(item["native"]["png"]), item["native"].get("rect"))
            web = load(p(item["web"]["png"]), item["web"].get("rect"))
            r = check_region(nat, kind, theme)
            if r: return iid, r
            na, wa = arr(nat), arr(web)
            d = np.abs(mean_rgb(na) - mean_rgb(wa)).max()
            if d > 24: return iid, f"colour: mean RGB differs by {d:.0f}/255 from the web"
            ln, lw = laplacian_var(na), laplacian_var(wa)
            if lw > 0 and ln < 0.5 * lw: return iid, f"blurry: Laplacian variance {ln:.1f} < 0.5 x web {lw:.1f}"
            diff = np.abs(small(nat) - small(web)).mean()
            if diff > item.get("max_diff", MAX_DIFF[kind]): return iid, f"differs from the web: mean abs diff {diff:.3f}"
            return iid, ""
        theme = item.get("theme", default_scheme)  # form A
        im = load(p(item["image"]), item.get("box"))
        r = check_region(im, kind, theme)
        if r: return iid, r
        if item.get("reference"):
            ref = load(p(item["reference"]))
            d = np.abs(mean_rgb(arr(im)) - mean_rgb(arr(ref))).max()
            if d > 24: return iid, f"colour: mean RGB of the foreground differs by {d:.0f}/255 from the reference (channels rotated or recoloured?)"
            diff = np.abs(small(im) - small(ref)).mean()
            if diff > item.get("max_diff", MAX_DIFF[kind]): return iid, f"differs from the reference: mean abs diff {diff:.3f}"
        return iid, ""
    except Exception as e:
        return iid, f"error: {type(e).__name__}: {e}"

def run(manifest_path, out=None):
    base = os.path.dirname(os.path.abspath(manifest_path))
    m = json.load(open(manifest_path))
    results = [run_item(it, base, m.get("scheme", "dark")) for it in m["items"]]
    bad = 0
    for iid, r in results:
        print(("FAIL " if r else "PASS ") + iid + (" " + r if r else " ok"))
        bad += bool(r)
    if out: json.dump([{"id": i, "ok": not r, "reason": r} for i, r in results], open(out, "w"), indent=1)
    return bad

# ---- self tests: deliberately broken inputs must FAIL -------------------------------------------
def synth_logo(theme, size=96):
    bg, fg = LOGO[theme]; im = Image.new("RGB", (size, size), bg)
    from PIL import ImageDraw
    d = ImageDraw.Draw(im); s = size
    d.polygon([(s*.2, s*.7), (s*.5, s*.2), (s*.8, s*.7), (s*.5, s*.55)], fill=fg); d.rectangle([s*.45, s*.6, s*.55, s*.85], fill=fg)
    return im
def synth_cat(coat=(214, 150, 80), panel=(0x13, 0x16, 0x1b), size=96):
    from PIL import ImageDraw
    im = Image.new("RGB", (size, size), panel); d = ImageDraw.Draw(im); s = size
    d.ellipse([s*.2, s*.3, s*.8, s*.9], fill=coat); d.polygon([(s*.25, s*.4), (s*.3, s*.1), (s*.45, s*.3)], fill=coat)
    d.ellipse([s*.35, s*.5, s*.45, s*.6], fill=(80, 200, 120)); d.ellipse([s*.55, s*.5, s*.65, s*.6], fill=(80, 200, 120))
    return im

def selftest_bare():
    with tempfile.TemporaryDirectory() as t:
        def save(name, im): im.save(os.path.join(t, name)); return name
        cases = [  # (item, must_pass)
            ({"id": "good dark logo", "kind": "logo", "theme": "dark", "image": save("a.png", synth_logo("dark"))}, True),
            ({"id": "good light logo", "kind": "logo", "theme": "light", "image": save("b.png", synth_logo("light"))}, True),
            ({"id": "good cat", "kind": "cat", "theme": "dark", "image": save("c.png", synth_cat())}, True),
            ({"id": "blank box", "kind": "logo", "theme": "dark", "image": save("d.png", Image.new("RGB", (96, 96), LOGO["dark"][0]))}, False),
            ({"id": "dark logo labelled light", "kind": "logo", "theme": "light", "image": "a.png"}, False),
            ({"id": "light logo labelled dark", "kind": "logo", "theme": "dark", "image": "b.png"}, False),
            ({"id": "missing file", "kind": "logo", "theme": "dark", "image": "nope.png"}, False),
            ({"id": "cat on a flat fill", "kind": "cat", "theme": "dark", "image": save("e.png", Image.new("RGB", (96, 96), (214, 150, 80)))}, False),
            ({"id": "box outside image", "kind": "logo", "theme": "dark", "image": "a.png", "box": [50, 50, 200, 200]}, False),
            ({"id": "good cat vs its own reference", "kind": "cat", "theme": "dark", "image": "c.png", "reference": "c.png"}, True),
            ({"id": "cat with rotated channels vs reference", "kind": "cat", "theme": "dark", "image": save("g.png", Image.merge("RGB", tuple(synth_cat().split()[i] for i in (2, 0, 1)))), "reference": "c.png"}, False),
            ({"id": "cat differs from reference", "kind": "cat", "theme": "dark", "image": "c.png", "reference": save("f.png", synth_logo("light"))}, False),
        ]
        ok = True
        for item, want in cases:
            iid, r = run_item(item, t)
            good = (not r) == want; ok &= good
            print(("selftest ok   " if good else "selftest WRONG ") + f"{iid}: {'pass' if not r else 'fail: ' + r}")
        return 0 if ok else 1

def selftest_manifest(path):
    """Mutate one native region at a time (blur, blank, recolour); each must FAIL, the original must PASS."""
    base = os.path.dirname(os.path.abspath(path)); m = json.load(open(path)); ok = True
    for item in m["items"][:4]:
        iid, r = run_item(item, base, m.get("scheme", "dark"))
        if r: print(f"selftest SKIP {iid}: original already fails ({r})"); ok = False; continue
        form_b = "native" in item
        ref = item["native"] if form_b else item
        pngkey = "png" if form_b else "image"; rectkey = "rect" if form_b else "box"
        for how in ("blur", "blank", "recolour"):
            with tempfile.TemporaryDirectory() as t:
                src = load(ref[pngkey] if os.path.isabs(ref[pngkey]) else os.path.join(base, ref[pngkey]))
                rect = ref.get(rectkey) or [0, 0, src.width, src.height]
                x, y, w, h = rect; region = src.crop((x, y, x + w, y + h))
                if how == "blur": new = region.resize((max(1, w // 6), max(1, h // 6)), Image.BILINEAR).resize((w, h), Image.BILINEAR).filter(ImageFilter.GaussianBlur(1.5))
                elif how == "blank": new = Image.new("RGB", (w, h), tuple(int(v) for v in dominant(arr(region))))
                else: new = Image.eval(region, lambda v: 255 - v) if True else region
                mutated = src.copy(); mutated.paste(new, (x, y)); mp = os.path.join(t, "m.png"); mutated.save(mp)
                it = copy.deepcopy(item); (it["native"] if form_b else it)[pngkey] = mp
                _, r2 = run_item(it, base, m.get("scheme", "dark"))
                good = bool(r2); ok &= good
                print(("selftest ok   " if good else "selftest WRONG ") + f"{iid} {how}: {'FAIL as required: ' + r2 if r2 else 'passed but must fail'}")
    return 0 if ok else 1

def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    out = argv[argv.index("--out") + 1] if "--out" in argv else None
    if out in args: args.remove(out)
    if "--selftest" in argv: return selftest_manifest(args[0]) if args else selftest_bare()
    if not args: print(__doc__); return 2
    return 1 if run(args[0], out) else 0

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
