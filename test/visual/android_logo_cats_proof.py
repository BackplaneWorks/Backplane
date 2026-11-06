#!/usr/bin/env python3
"""Proof that check_logo_cats.py fails on broken Android inputs and passes on the real ones.
Needs build/gpui/shots/logo-cats/{raw,ref} (scripts/android-logo-cats-shots.sh). usage: android_logo_cats_proof.py
Builds broken manifests in a temp dir from the real screenshots and requires each to FAIL for its reason; then requires
test/visual/android-logo-cats.manifest.json to PASS. Exit 0 only if everything behaves."""
import json, os, shutil, subprocess, sys, tempfile
from PIL import Image, ImageOps
here = os.path.dirname(os.path.abspath(__file__)); root = os.path.abspath(here + "/../..")
chk = here + "/check_logo_cats.py"; real = here + "/android-logo-cats.manifest.json"
M = json.load(open(real)); byid = {i["id"]: i for i in M["items"]}
def src(i): return os.path.normpath(os.path.join(here, i["image"])), os.path.normpath(os.path.join(here, i["reference"]))
tmp = tempfile.mkdtemp(prefix="logocats-")
def run(items, name):
    mf = f"{tmp}/{name}.json"; json.dump({"items": items}, open(mf, "w"))
    p = subprocess.run([sys.executable, chk, mf], capture_output=True, text=True); return p.returncode, p.stdout.strip()
def copy(path, tag):
    d = f"{tmp}/{tag}.png"; shutil.copy(path, d); return d
bad = []
# 1. the light logo, but the Android header shows the DARK logo (wrong theme for the colours)
l = byid["logo-dark"]; img, ref = src(l); bad.append(("wrong-theme logo (dark art in a light theme)", dict(l, id="x-wrong-theme", theme="light", image=copy(img, "a"), reference=copy(ref, "a-ref")), "FAIL"))
# 2. a logo box over an empty part of the screen: missing/blank logo
img, ref = src(l); bad.append(("missing logo (blank box)", dict(l, id="x-blank-logo", image=copy(img, "b"), reference=copy(ref, "b-ref"), box=[500, 1500, 47, 47]), "FAIL"))
# 3. a cat that is not drawn (box over empty space)
c = byid["cat-row-Bub-dark"]; img, ref = src(c); bad.append(("missing cat (blank box)", dict(c, id="x-blank-cat", image=copy(img, "c"), reference=copy(ref, "c-ref"), box=[500, 1700, 74, 74]), "FAIL"))
# 4. a wrong-coloured cat: the box's colours inverted (a clearly wrong coat)
im = Image.open(img).convert("RGB"); x, y, w, h = c["box"]
inv = ImageOps.invert(im.crop((x, y, x + w, y + h))); im2 = im.copy(); im2.paste(inv, (x, y)); d = f"{tmp}/d.png"; im2.save(d)
bad.append(("wrong-coloured cat (colours inverted)", dict(c, id="x-wrong-cat", image=d, reference=copy(ref, "d-ref")), "FAIL"))
# 4b. informational: channels rotated (a subtler recolour); the reference check alone may let it through (see GAP line)
r, g, b2 = im.crop((x, y, x + w, y + h)).split(); im3 = im.copy(); im3.paste(Image.merge("RGB", (b2, r, g)), (x, y)); d3 = f"{tmp}/d3.png"; im3.save(d3)
# 5. a missing screenshot
bad.append(("missing image file", dict(l, id="x-missing", image=f"{tmp}/does-not-exist.png", reference=copy(ref, "e-ref")), "FAIL"))
# 6. a flat-filled cat
flat = Image.new("RGB", Image.open(img).size, (40, 44, 52)); fp = f"{tmp}/f.png"; flat.save(fp)
bad.append(("flat cat (no art)", dict(c, id="x-flat-cat", image=fp, reference=copy(ref, "f-ref")), "FAIL"))
ok = True
for name, item, want in bad:
    rc, out = run([item], item["id"]); got = "FAIL" if rc else "PASS"
    print(f"{'ok ' if got == want else 'BAD'} {name}: expected {want}, got {got}: {out.splitlines()[-1] if out else ''}"); ok &= got == want
rc3, out3 = run([dict(c, id="x-rotated-cat", image=d3, reference=copy(ref, "d3-ref"))], "rot")
print(("ok  " if rc3 else "GAP ") + "channel-rotated cat (subtle recolour): " + out3.splitlines()[-1] + ("" if rc3 else "   <- check_logo_cats.py does not yet compare mean colour for form A references"))
rc, out = subprocess.run([sys.executable, chk, real], capture_output=True, text=True).returncode, ""
print(f"{'ok ' if rc == 0 else 'BAD'} the real Android manifest ({len(M['items'])} items): {'PASS' if rc == 0 else 'FAIL'}"); ok &= rc == 0
st = subprocess.run([sys.executable, chk, "--selftest"], capture_output=True, text=True)
print(f"{'ok ' if st.returncode == 0 else 'BAD'} the checker's own --selftest: exit {st.returncode}"); ok &= st.returncode == 0
shutil.rmtree(tmp); sys.exit(0 if ok else 1)
