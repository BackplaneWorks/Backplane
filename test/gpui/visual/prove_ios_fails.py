#!/usr/bin/env python3
"""Proof that the iOS logo/cats check fails on broken input (run after make_manifest_ios.py).
usage: prove_ios_fails.py MANIFEST.json   -> prints each break and the check's verdict; exit 1 if a break PASSES.
Breaks: (blank) the native logo replaced by a flat background; (wrong colours) the light logo where the dark
one belongs; (missing cat) a cat rectangle moved onto empty screen; (blurry) a blurred copy of the native shot."""
import copy, json, os, subprocess, sys, tempfile
from PIL import Image, ImageFilter
man = sys.argv[1]; base = os.path.dirname(os.path.abspath(man)); m = json.load(open(man))
check = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check_logo_cats.py")
tmp = tempfile.mkdtemp(prefix="logocats-")
fails = 0


def run(name, mm, want):
    global fails
    path = os.path.join(tmp, name + ".json"); json.dump(mm, open(path, "w"))
    r = subprocess.run([sys.executable, check, path], capture_output=True, text=True)
    bad = [l for l in r.stdout.splitlines() if l.startswith("FAIL") and want in l]
    print(f"{name}: exit {r.returncode}; {bad[0] if bad else 'NO MATCHING FAIL: ' + r.stdout[:200]}")
    if r.returncode != 1 or not bad:
        fails += 1


def item(mm, kind):
    return next(i for i in mm["items"] if i["kind"] == kind)


def absolutize(mm):
    for i in mm["items"]:
        for s in ("web", "native"):
            i[s]["png"] = os.path.normpath(os.path.join(base, i[s]["png"]))
    return mm


# blank: paint the native logo's rectangle flat
mm = absolutize(copy.deepcopy(m)); it = item(mm, "logo")
im = Image.open(it["native"]["png"]).convert("RGB"); x, y, w, h = it["native"]["rect"]
im.paste(im.getpixel((max(0, x - 8), max(0, y - 8))), (x, y, x + w, y + h)); p = os.path.join(tmp, "blank.png"); im.save(p); it["native"]["png"] = p
run("blank", mm, "blank")
# wrong colours: invert the native logo's rectangle (a dark logo on a light theme and the reverse)
mm = absolutize(copy.deepcopy(m)); it = item(mm, "logo")
im = Image.open(it["native"]["png"]).convert("RGB"); x, y, w, h = it["native"]["rect"]
from PIL import ImageOps
im.paste(ImageOps.invert(im.crop((x, y, x + w, y + h))), (x, y)); p = os.path.join(tmp, "inverted.png"); im.save(p); it["native"]["png"] = p
run("wrong-colours", mm, "colour")
# missing cat: the rectangle sits on empty screen far from any cat
mm = absolutize(copy.deepcopy(m)); it = item(mm, "cat"); x, y, w, h = it["native"]["rect"]
im = Image.open(it["native"]["png"]).convert("RGB"); it["native"]["rect"] = [im.width - w - 4, im.height // 2, w, h]
run("missing-cat", mm, "")
# blurry: a rasterised-and-stretched logo (render at 1/6 size, scale back up, blur)
mm = absolutize(copy.deepcopy(m)); it = item(mm, "logo")
im = Image.open(it["native"]["png"]).convert("RGB"); x, y, w, h = it["native"]["rect"]
c = im.crop((x, y, x + w, y + h)); c = c.resize((max(1, w // 6), max(1, h // 6)), Image.BILINEAR).resize((w, h), Image.BILINEAR).filter(ImageFilter.GaussianBlur(1.5))
im.paste(c, (x, y)); p = os.path.join(tmp, "blur.png"); im.save(p); it["native"]["png"] = p
run("blurry", mm, "")
print("all breaks fail as required" if not fails else f"{fails} break(s) were NOT caught")
sys.exit(1 if fails else 0)
