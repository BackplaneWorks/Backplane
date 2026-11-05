#!/usr/bin/env python3
"""The web reference for the logo shots: build/gpui/web is a stale bundle (before #922, commit 80cc30e499), so
copy it and apply that commit's change to its css (brand-mark background -> var(--brand-image), apple-touch-icon.png
for dark, favicon-light.svg for light). usage: make_webref.py WEB_DIR OUT_DIR  (serve OUT_DIR with `backplane-serve --web`)"""
import base64, re, shutil, sys, os
src, out = sys.argv[1:3]
repo = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../..")
shutil.rmtree(out, ignore_errors=True); shutil.copytree(src, out)
for f in os.listdir(out):
    if f.endswith(".gz"): os.remove(os.path.join(out, f))
cssf = next(os.path.join(out, f) for f in os.listdir(out) if f.endswith(".css"))
css = open(cssf).read()
css, n = re.subn(r'url\(data:image/png;base64,[A-Za-z0-9+/=]+\) 50%/contain no-repeat', 'var(--brand-image) center/contain no-repeat', css)
assert n >= 1, "no brand-mark rule found"
png = base64.b64encode(open(f"{repo}/src/web/apple-touch-icon.png", "rb").read()).decode()
css = css.replace(":root{", ":root{--brand-image:url(data:image/png;base64,%s);" % png, 1)
a = "html.light,.app.light,.bub-move.light{"
assert a in css
css = css.replace(a, a + "--brand-image:url(favicon-light.svg);", 1)
open(cssf, "w").write(css)
shutil.copy(f"{repo}/src/web/favicon-light.svg", out)
print("web reference with #922 written to", out)
