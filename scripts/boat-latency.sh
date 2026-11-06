#!/bin/sh
# Click and popup latency of the GPUI client against the Rust hub, on Boat (Xvfb, llvmpipe = software rendering; NOT the RTX).
# Needs scripts/boat-build.sh first (the client) and the Rust hub binary (HUB_BIN, default test/hubbin/backplane-hub, uploaded with
# --include-file). Synthetic events from test/perf/gen_log.py (EVENTS, default 6000: about 40 threads). Output: build/gpui-latency/.
set -u
V=build/gpui-latency; mkdir -p $V; : > $V/results.txt
BIN=$PWD/build/gpui-target/release/backplane-gpui; export BACKPLANE_GPUI_ASSETS=$PWD/build/gpui-artifacts/assets
HUB=${HUB_BIN:-$PWD/test/hubbin/backplane-hub}; chmod +x test/hubbin/* 2>/dev/null
say() { echo "$*" | tee -a $V/results.txt; }
need() { command -v "$1" >/dev/null 2>&1; }
if ! need Xvfb || ! need i3; then (sudo -n apt-get update -qq && sudo -n apt-get install -y -qq xvfb i3 x11-xserver-utils mesa-vulkan-drivers) > $V/apt.log 2>&1 || say "apt install failed"; fi
H=$PWD/$V/home; mkdir -p $H
python3 test/perf/gen_log.py $H/events.jsonl ${EVENTS:-6000} 7
"$HUB" --home $H --port 3830 --host 127.0.0.1 --no-tailscale --foreground --window-bin $PWD/test/hubbin/window > $V/hub.log 2>&1 & HP=$!
sleep 4; say "hub pid $HP up: $(curl -s -m3 -o /dev/null -w '%{http_code}' http://127.0.0.1:3830/hello)  events: $(wc -l < $H/events.jsonl)"
python3 - <<'PY' > $V/script.json
import json
steps = []
for rnd in range(3):
    for k in range(12):
        steps += [{"select": k}, {"wait": 250}, {"popup": "/"}, {"wait": 250}]
json.dump(steps, open("build/gpui-latency/script.json", "w"))
print(json.dumps(steps))
PY
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > $V/i3.conf
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
Xvfb :61 -screen 0 1440x900x24 -nolisten tcp > $V/xvfb.log 2>&1 & XP=$!
sleep 2; DISPLAY=:61 i3 -c $V/i3.conf > $V/i3.log 2>&1 & IP=$!
printf 'Xft.dpi: 96\nXft.antialias: 1\nXft.hinting: 1\nXft.hintstyle: hintslight\nXft.rgba: none\n' | DISPLAY=:61 xrdb -merge
sleep 1
DISPLAY=:61 "$BIN" --url http://127.0.0.1:3830 --token-file $H/token --script $V/script.json --exit-after 150 > $V/client.log 2>&1
kill $IP $XP $HP 2>/dev/null; wait 2>/dev/null
grep -m1 '"event":"gpu"' $V/client.log | cut -c1-200 | sed 's/^/adapter: /' | tee -a $V/results.txt
python3 test/perf/latency_summary.py $V/client.log $V/latency.json | tee -a $V/results.txt
exit 0
