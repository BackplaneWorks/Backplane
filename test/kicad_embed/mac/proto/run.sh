#!/bin/sh
# Option (b) measured: a borderless window of another (accessory) process kept over a moving window's pane.
# On a Mac with a logged-in GUI session (the e2e-macos runner; on hs-mac-mini under the hub lock mac-ui). Needs cliclick for the drag.
set -eu
cd "$(dirname "$0")"; O=${1:-/tmp/lane-m-proto}; rm -rf "$O"; mkdir -p "$O"
app() { A=$O/$1.app; mkdir -p $A/Contents/MacOS
  clang -fobjc-arc -O2 -framework Cocoa -framework QuartzCore -o $A/Contents/MacOS/$1 $1.m
  printf '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>%s</string><key>CFBundleIdentifier</key><string>works.backplane.lane-m.%s</string><key>CFBundlePackageType</key><string>APPL</string><key>NSHighResolutionCapable</key><true/></dict></plist>' $1 $1 > $A/Contents/Info.plist
  codesign --force --sign - $A >/dev/null 2>&1 || true; }
for p in host follower sampler cover; do app $p; done
waitline() { i=0; while [ ! -s "$1" ] && [ $i -lt 200 ]; do sleep 0.05; i=$((i+1)); done; head -1 "$1"; }
start() { # name scenario counter secs
  n=$1; open -n --stdout "$O/host.$n.out" --stderr "$O/host.$n.err" "$O/host.app" --args $2 "$O/fid.$n" $3
  HID=$(waitline "$O/host.$n.out")
  open -n --stdout "$O/fol.$n.out" --stderr "$O/fol.$n.err" "$O/follower.app" --args "$HID" $4 poll
  FID=$(waitline "$O/fol.$n.out"); echo $FID > "$O/fid.$n"; }
# 1, 2: programmatic move/resize, reactivation, full screen; without and with the host's own counter-order
for c in plain counter; do
  start move.$c move $c 15
  ( sleep 5.8; open -n --stdout "$O/cov.move.$c" "$O/cover.app" --args 300 250 1.0 ) &
  "$O/sampler.app/Contents/MacOS/sampler" "$HID" "$FID" 15 "$O/cov.move.$c" > "$O/samples.move.$c.csv"
  wait; sleep 0.5
  echo "move.$c host=$HID follower=$FID $(grep -h -v '^[0-9]*$' "$O/host.move.$c.out" "$O/fol.move.$c.out" | tr '\n' ' ')"
done
# 3: a real title-bar drag with the mouse (cliclick), then another app put over part of the host
if command -v cliclick >/dev/null; then
  start drag still counter 21
  "$O/sampler.app/Contents/MacOS/sampler" "$HID" "$FID" 14 "$O/cov.drag" > "$O/samples.drag.csv" &
  S=$!; sleep 1
  # title bar of a window at (40, H-150-400) in Cocoa coords: CG y = 768-550-... read it from the sampler instead
  read -r hx hy <<XY
$(tail -1 "$O/samples.drag.csv" | awk -F, '{print $2+100, $3+12}')
XY
  cliclick dd:$hx,$hy
  i=0; while [ $i -lt 60 ]; do i=$((i+1)); cliclick -w 16 m:$((hx + i*5)),$((hy + i*2)); done
  cliclick du:$((hx + 300)),$((hy + 120))
  sleep 1; open -n --stdout "$O/cov.drag" "$O/cover.app" --args 200 200 4
  sleep 2; screencapture -x "$O/covered.png" || true
  wait $S; sleep 0.5
  echo "drag host=$HID follower=$FID $(grep -h -v '^[0-9]*$' "$O/fol.drag.out" | tr '\n' ' ')"
fi
pkill -f "$O/" 2>/dev/null || true; sleep 0.3
pgrep -lf "$O/" || echo "nothing left running"
