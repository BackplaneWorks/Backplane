#!/bin/sh
# One short real-GPU latency run on the desktop: the Boat-built client on an isolated Xvfb display (:61, never :0) with the NVIDIA ICD, against a hub
# already running on 127.0.0.1:3860 (scripts/try.sh start with GPUI_TRY_NOAPP=1). Everything of ours runs inside one scope,
# and is killed by captured PIDs at the end. Output: build/gpui/results/desktop/rtx/ (metrics only: no screenshots, no thread text).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd); OUT=$ROOT/build/gpui/results/desktop/rtx; mkdir -p $OUT
if [ -z "${RTX_INNER:-}" ]; then
  RTX_INNER=1 exec systemd-run --user --scope -q sh "$0" "$@"
fi
X=$ROOT/build/gpui/xvfb
python3 - > $OUT/script.json <<'PY'
import json
steps = []
for rnd in range(3):
    for k in range(12):
        steps += [{"select": k}, {"wait": 400}, {"popup": "/"}, {"wait": 400}]
print(json.dumps(steps))
PY
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > $OUT/i3.conf
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb :61 -screen 0 1440x900x24 -xkbdir /usr/share/X11/xkb -nolisten tcp > $OUT/xvfb.log 2>&1 & XP=$!
sleep 2; DISPLAY=:61 i3 -c $OUT/i3.conf > $OUT/i3.log 2>&1 & IP=$!
printf 'Xft.dpi: 96\nXft.antialias: 1\nXft.hinting: 1\nXft.hintstyle: hintslight\nXft.rgba: none\n' | DISPLAY=:61 xrdb -merge
sleep 1
env BACKPLANE_GPUI_ASSETS=$ROOT/build/gpui/linux/assets VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json DISPLAY=:61 \
  $ROOT/build/gpui/linux/backplane-gpui --url http://127.0.0.1:3860 --token-file $HOME/.backplane-gpui-try/home/token --script $OUT/script.json --exit-after 150 > $OUT/client.log 2>&1
kill $IP $XP 2>/dev/null; wait $IP $XP 2>/dev/null
grep -m1 '"event":"gpu"' $OUT/client.log | cut -c1-220
