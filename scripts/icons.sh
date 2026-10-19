#!/bin/sh
# Draw the app icon everywhere from assets/icon/ (the backplane.works mark):
#   backplane.svg  the icon: the mark on #111410
#   backplane-desktop.svg  the desktop entry's icon: the mark on a rounded tile
#   mark.svg       the mark alone (Android's layers are vectors of it)
# Writes the iOS app icon, the web's favicon and touch icon, the desktop
# PNGs (packaging) and the window's icon rows in src/app/effects/win.c.
# Needs rsvg-convert and ImageMagick; run it after changing the SVGs.
set -eu
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# the iOS icon may have no alpha channel
rsvg-convert -w 1024 -h 1024 assets/icon/backplane.svg -o "$tmp/ios.png"
magick "$tmp/ios.png" -alpha off mobile/ios/Backplane/Assets.xcassets/AppIcon.appiconset/icon.png
cp assets/icon/backplane.svg src/web/favicon.svg
rsvg-convert -w 180 -h 180 assets/icon/backplane.svg -o src/web/apple-touch-icon.png
for s in 64 128 256 512; do
  rsvg-convert -w $s -h $s assets/icon/backplane.svg -o assets/icon/backplane-$s.png
  rsvg-convert -w $s -h $s assets/icon/backplane-desktop.svg -o assets/icon/backplane-desktop-$s.png
done
# the window's icon (_NET_WM_ICON): the mark's coverage at 64 px, one hex
# digit a pixel, laid over the background by win.c
rsvg-convert -w 64 -h 64 assets/icon/backplane.svg -b '#000000' -o "$tmp/w.png"
magick "$tmp/w.png" -colorspace gray -depth 8 gray:"$tmp/w.gray"
python3 - "$tmp/w.gray" src/app/effects/win.c <<'EOF'
import sys
px = open(sys.argv[1], "rb").read()
# #111410's gray is the floor, #e6e9de's the top
lo, hi = 0x13, 0xe7
rows = []
for y in range(64):
    r = px[y * 64:(y + 1) * 64]
    rows.append('  "' + "".join("%x" % max(0, min(15, round((b - lo) * 15 / (hi - lo)))) for b in r) + '"')
block = "static const char* win_icon_rows[64] = {\n" + ",\n".join(rows) + "\n};\n"
path = sys.argv[2]
src = open(path).read()
a = src.index("// icon rows (scripts/icons.sh)\n") + len("// icon rows (scripts/icons.sh)\n")
b = src.index("// icon rows end\n")
open(path, "w").write(src[:a] + block + src[b:])
EOF
ls -la assets/icon mobile/ios/Backplane/Assets.xcassets/AppIcon.appiconset/icon.png src/web/favicon.svg src/web/apple-touch-icon.png
