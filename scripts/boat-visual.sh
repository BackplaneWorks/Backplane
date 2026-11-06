#!/bin/sh
# Visual checks on Boat (software rendering: llvmpipe), after scripts/boat-build.sh built the binary and assets.
# Never run on the desktop. Writes build/gpui-visual/ (PNGs, logs, results.txt). Xft settings mirror the real display:
# Xft.dpi 96, hinting 1, hintslight, rgba none, antialias 1 (scale 2 doubles the dpi).
set -u
V=build/gpui-visual; mkdir -p $V; : > $V/results.txt
BIN=$PWD/build/gpui-target/release/backplane-gpui; export BACKPLANE_GPUI_ASSETS=$PWD/build/gpui-artifacts/assets
say() { echo "$*" | tee -a $V/results.txt; }
need() { command -v "$1" >/dev/null 2>&1; }
if ! need Xvfb || ! need i3 || ! need import; then
  (sudo -n apt-get update -qq && sudo -n apt-get install -y -qq xvfb i3 imagemagick x11-xserver-utils x11-utils mesa-vulkan-drivers) > $V/apt.log 2>&1 || say "apt install failed (see apt.log)"
fi
python3 -m pip install --user -q pillow cairosvg numpy > $V/pip.log 2>&1 || python3 -m pip install --user -q --break-system-packages pillow cairosvg numpy >> $V/pip.log 2>&1 || say "pip install failed"
say "tools: Xvfb=$(need Xvfb && echo yes || echo no) i3=$(need i3 && echo yes || echo no) import=$(need import && echo yes || echo no) lvp=$(ls /usr/share/vulkan/icd.d/lvp_icd.json 2>/dev/null || echo none)"
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\ndefault_floating_border none\nhide_edge_borders both\n' > $V/i3.conf
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
shoot() { # name display W H dpi flags...
  n=$1; d=$2; w=$3; h=$4; dpi=$5; shift 5
  Xvfb $d -screen 0 ${w}x${h}x24 -nolisten tcp > $V/xvfb$d.log 2>&1 & XP=$!
  sleep 2; DISPLAY=$d i3 -c $V/i3.conf > $V/i3$d.log 2>&1 & IP=$!
  printf 'Xft.dpi: %s\nXft.antialias: 1\nXft.hinting: 1\nXft.hintstyle: hintslight\nXft.rgba: none\n' $dpi | DISPLAY=$d xrdb -merge
  sleep 1
  DISPLAY=$d "$BIN" --url http://127.0.0.1:1 --token-file /dev/null --exit-after 9 "$@" > $V/$n.log 2>&1 & GP=$!
  sleep 6; DISPLAY=$d import -window root $V/$n.png; wait $GP
  kill $IP $XP 2>/dev/null; wait $IP $XP 2>/dev/null
  grep -m1 '"event":"gpu"' $V/$n.log | cut -c1-200 | sed "s/^/$n adapter: /" | tee -a $V/results.txt
}
rc=0
for s in dark light; do
  shoot swatches-$s :61 1440 900 96 --swatches --theme $s
  python3 test/visual/check_colors.py $V/swatches-$s.png $V/swatches-$s.log $s --adapter llvmpipe > $V/colors-$s.txt 2>&1 || rc=1
  say "colors $s: $(grep -c '^PASS' $V/colors-$s.txt) pass, $(grep -c '^FAIL' $V/colors-$s.txt) fail"
  shoot gallery-$s :61 1440 900 96 --gallery --theme $s
  python3 test/visual/icons_check.py $V/gallery-$s.png $V/gallery-$s.log $BACKPLANE_GPUI_ASSETS $s $V > $V/icons-prep-$s.txt 2>&1 || rc=1
  cp $V/manifest-icons.json $V/manifest-icons-$s.json; cp $V/icons-ref.png $V/icons-ref-$s.png
  python3 test/visual/check_logo_cats.py $V/manifest-icons-$s.json > $V/icons-$s.txt 2>&1 || rc=1
  say "icons $s: $(grep -c '^PASS' $V/icons-$s.txt) pass, $(grep -c '^FAIL' $V/icons-$s.txt) fail; $(cat $V/icons-prep-$s.txt | tail -1)"
  python3 test/visual/check_logo_cats.py $V/manifest-icons-$s.json --selftest > $V/icons-selftest-$s.txt 2>&1 || say "icons selftest $s did not behave"
  shoot fonts-$s :61 1440 900 96 --fonts --theme $s
  python3 test/visual/check_fonts.py $V/fonts-$s.png $V/fonts-$s.log assets/fonts/jetbrains-mono $s --out $V/fonts-side-$s.png > $V/fonts-$s.txt 2>&1 || rc=1
  say "fonts $s: $(grep -c '^PASS' $V/fonts-$s.txt) pass, $(grep -c '^FAIL' $V/fonts-$s.txt) fail"
done
python3 test/visual/check_fonts.py --selftest > $V/fonts-selftest.txt 2>&1 || say "font selftest did not behave"
python3 test/visual/check_colors.py --selftest > $V/colors-selftest.txt 2>&1 || say "colour selftest did not behave"
say "done rc=$rc"
exit 0
