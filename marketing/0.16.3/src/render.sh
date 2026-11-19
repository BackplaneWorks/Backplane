#!/bin/sh
# Renders graphic.html's pages to PNGs in the folder above, with a headless browser (no display is used; its profile is a temp folder).
# The typeface is Berkeley Mono (docs/STYLE_GUIDE.md): installed on the rendering machine, never committed. usage: render.sh [page...]
set -eu
D=$(cd "$(dirname "$0")" && pwd); OUT=$(dirname "$D"); CHROME=${CHROME:-$(command -v google-chrome || command -v chromium)}
BM=$(fc-list : file family | grep -i 'Berkeley Mono' | grep -m1 -i 'variable' | cut -d: -f1); [ -n "$BM" ] || BM=$(fc-list : file family | grep -m1 -i 'Berkeley Mono' | cut -d: -f1)
[ -n "$BM" ] || { echo "Berkeley Mono is not installed on this machine"; exit 2; }
printf '@font-face { font-family: "BP Brand"; src: url("file://%s"); font-weight: 100 900; }\n:root { --brand-type: "BP Brand", "Berkeley Mono", monospace; }\n' "$BM" > "$D/brand-type.css"
T=$(mktemp -d); trap 'rm -rf "$T" "$D/brand-type.css"' EXIT
shot() { # page name width height
  "$CHROME" --headless=new --disable-gpu --no-first-run --user-data-dir="$T/p" --hide-scrollbars --force-device-scale-factor=1 --window-size="$3,$4" \
    --virtual-time-budget=4000 --screenshot="$OUT/$2.png" "file://$D/graphic.html#$1" >/dev/null 2>&1
  echo "$OUT/$2.png"
}
for p in ${@:-1 2 3 4 5 wide banner}; do case $p in
  1) shot 1 backplane-0.16.3-1-hero-1080 1080 1080;;
  2) shot 2 backplane-0.16.3-2-altium-1080 1080 1080;;
  3) shot 3 backplane-0.16.3-3-snapshots-1080 1080 1080;;
  4) shot 4 backplane-0.16.3-4-copy-run-1080 1080 1080;;
  5) shot 5 backplane-0.16.3-5-terminal-1080 1080 1080;;
  wide) shot wide backplane-0.16.3-hero-1600x900 1600 900;;
  banner) shot banner backplane-0.16.3-banner-1500x500 1500 500;;
esac; done
