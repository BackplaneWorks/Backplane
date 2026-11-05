#!/bin/sh
# Why did the e2e render job show a black screen? Same setup as test/gpui/e2e/lib/capture.mjs (Xvfb :62, the e2e i3 conf, process env + lavapipe),
# variants: A plain, B HOME=a missing dir (the harness sets HOME to the hub's fakehome), C against the Rust hub without its policy helper (canned presenter).
# Needs test/gpui/hubbin/{gpui,assets,backplane-hub} (untracked). Output build/gpui-render/ (stderr, screenshot, RSS per variant).
set -u
[ "$(id -un)" = user ] || { echo "boat-render-check.sh: not a Boat sandbox (user $(id -un)); refusing" >&2; exit 3; }
V=build/gpui-render; mkdir -p $V; : > $V/results.txt
say() { echo "$*" | tee -a $V/results.txt; }
need() { command -v "$1" >/dev/null 2>&1; }
if ! need Xvfb || ! need i3 || ! need import; then (sudo -n apt-get update -qq && sudo -n apt-get install -y -qq xvfb i3 imagemagick x11-xserver-utils x11-utils mesa-vulkan-drivers) > $V/apt.log 2>&1 || say "apt failed"; fi
BIN=$PWD/test/gpui/hubbin/gpui; chmod +x $BIN test/gpui/hubbin/backplane-hub 2>/dev/null
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\ndefault_floating_border none\nhide_edge_borders both\n' > /tmp/e2e-i3.conf
run() { # name, extra env, url, token
  n=$1; shift
  Xvfb :62 -screen 0 1440x900x24 -nolisten tcp > $V/$n.xvfb 2>&1 & XP=$!; sleep 1.5
  DISPLAY=:62 i3 -c /tmp/e2e-i3.conf > $V/$n.i3 2>&1 & IP=$!; sleep 1.5
  env DISPLAY=:62 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json BACKPLANE_GPUI_ASSETS=$PWD/test/gpui/hubbin/assets "$@" > $V/$n.log 2>&1 & GP=$!
  sleep 12; RSS=$(grep VmRSS /proc/$GP/status 2>/dev/null | tr -s ' '); DISPLAY=:62 import -window root $V/$n.png
  say "$n: pid alive=$(kill -0 $GP 2>/dev/null && echo yes || echo no) $RSS; first_frame=$(grep -c first_frame $V/$n.log) process_start=$(grep -c process_start $V/$n.log)"
  kill $GP 2>/dev/null; kill $IP $XP 2>/dev/null; wait $GP $IP $XP 2>/dev/null
}
mkdir -p /tmp/h-token && echo tok0123456789abcdef0123456789abcdef > /tmp/h-token/token
run A $BIN --url http://127.0.0.1:1 --token-file /tmp/h-token/token --exit-after 20
run B HOME=/nonexistent/fakehome $BIN --url http://127.0.0.1:1 --token-file /tmp/h-token/token --exit-after 20
H=$PWD/$V/hubhome; mkdir -p $H; python3 test/gpui/perf/gen_log.py $H/events.jsonl 3000 7
test/gpui/hubbin/backplane-hub --home $H --port 3831 --host 127.0.0.1 --no-tailscale --foreground > $V/hub-canned.log 2>&1 & HP=$!; sleep 3
run C $BIN --url http://127.0.0.1:3831 --token-file $H/token --exit-after 20
kill $HP 2>/dev/null
exit 0
