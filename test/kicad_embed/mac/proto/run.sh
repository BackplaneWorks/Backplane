#!/bin/sh
# Option (b) measured: a borderless window of another process kept over a window's pane (see host.m, follower.m).
# Needs a Mac with a logged-in GUI session (the e2e-macos runner; hs-mac-mini under the hub lock mac-ui) and cliclick.
set -u
cd "$(dirname "$0")"; O=${1:-/tmp/lane-m-proto}; rm -rf "$O"; mkdir -p "$O"
app() { A=$O/$1.app; mkdir -p $A/Contents/MacOS
  clang -fobjc-arc -O2 -framework Cocoa -framework QuartzCore -o $A/Contents/MacOS/$1 $1.m || exit 1
  printf '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>%s</string><key>CFBundleIdentifier</key><string>works.backplane.lane-m.%s</string><key>CFBundleName</key><string>%s</string><key>CFBundlePackageType</key><string>APPL</string><key>NSHighResolutionCapable</key><true/></dict></plist>' $1 $1 $1 > $A/Contents/Info.plist
  codesign --force --sign - $A >/dev/null 2>&1 || true; }
for p in host follower sampler cover; do app $p; done
waitline() { i=0; while [ ! -s "$1" ] && [ $i -lt 200 ]; do sleep 0.05; i=$((i+1)); done; head -1 "$1"; }
now() { python3 -c 'import time; print("%.3f" % time.time())'; }
# start <name> <scenario> <activation> <mode> <policy>: sets HID HPID FID
start() { n=$1; S=$O/sock.$n
  open -n --stdout "$O/host.$n.out" --stderr "$O/host.$n.err" "$O/host.app" --args $2 "$S" $3
  set -- $(waitline "$O/host.$n.out") ; HID=$1; HPID=$2
  open -n --stdout "$O/fol.$n.out" --stderr "$O/fol.$n.err" "$O/follower.app" --args $4 "$HID" "$HPID" "$S" $5 25
  FID=$(waitline "$O/fol.$n.out"); }
finish() { pkill -f "$O/host.app" 2>/dev/null; pkill -f "$O/follower.app" 2>/dev/null; pkill -f "$O/cover.app" 2>/dev/null; sleep 0.5; }
# 1. the scripted timeline: polling (first prototype) and push with each activation variant
for v in poll:none push:none push:below push:willbelow push:tell push:both; do
  m=${v%%:*}; a=${v#*:}; n=move.$m.$a
  start $n move $a $m accessory
  ( sleep 5.6; open -n --stdout "$O/cov.$n" "$O/cover.app" --args 300 250 1.0 ) &
  "$O/sampler.app/Contents/MacOS/sampler" "$HID" "$FID" 16.5 "$O/cov.$n" > "$O/samples.$n.csv"
  wait; finish
  echo "$n: $(tail -n +2 "$O/host.$n.out" | awk '{print $NF}' | tr '\n' ' ')"
done
# 2. a real title-bar drag with the mouse, push mode, fine steps
start drag still both push accessory
"$O/sampler.app/Contents/MacOS/sampler" "$HID" "$FID" 6 > "$O/samples.drag.csv" & SP=$!
sleep 1.2; set -- $(tail -1 "$O/samples.drag.csv" | awk -F, '{print $2+150, $3+12}'); hx=$1; hy=$2
cliclick dd:$hx,$hy; i=0; while [ $i -lt 100 ]; do i=$((i+1)); cliclick m:$((hx + i*3)),$((hy + i)); done; cliclick du:$((hx+300)),$((hy+100))
wait $SP; finish
# 3. the host killed: how long its follower stays on screen
start death still both push accessory
sleep 1.5; t=$(now); kill -9 $HPID; sleep 1
echo "death: killed at $t, follower: $(grep 'host gone' "$O/fol.death.out")"
pgrep -f "$O/follower.app" >/dev/null && echo "death: FOLLOWER STILL RUNNING" || echo "death: follower gone"
finish
# 4. the menu bar and the Dock with each policy: click into the follower, see what has the keyboard and the menu bar
for p in accessory regular switch; do
  start menu.$p still both push $p
  sleep 1.2; set -- $(osascript -e 'tell application "System Events" to get position of window 1 of process "host"' | tr -d ','); 
  fx=$(( $1 + 400 )); fy=$(( $2 + 250 ))
  cliclick c:$fx,$fy; sleep 0.8
  front=$(osascript -e 'tell application "System Events" to get name of first process whose frontmost is true' 2>&1)
  menus=$(osascript -e 'tell application "System Events" to get name of every menu bar item of menu bar 1 of (first process whose frontmost is true)' 2>&1)
  dock=$(osascript -e 'tell application "System Events" to get name of every UI element of list 1 of process "Dock"' 2>&1 | tr ',' '\n' | grep -c -i follower)
  cmdtab=$(osascript -e 'tell application "System Events" to get name of every process whose background only is false' 2>&1 | tr ',' '\n' | grep -c -i follower)
  screencapture -x "$O/menu.$p.png"
  # Cmd-click back into the host's own area (left of the pane): does the host get the keyboard back?
  cliclick c:$(( fx - 300 )),$fy; sleep 0.5
  back=$(osascript -e 'tell application "System Events" to get name of first process whose frontmost is true' 2>&1)
  echo "menu.$p: after a click in the follower the frontmost is '$front'; menu bar: $menus; follower in the Dock: $dock; in the app list (Cmd-Tab): $cmdtab; follower key: $(grep -c '^key' "$O/fol.menu.$p.out"); after a click back on the host: '$back'"
  finish
done
pgrep -lf "$O/" || echo "nothing left running"
