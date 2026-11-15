#!/bin/sh
# On the Mac, under the hub lock mac-ui. Builds the three stand-ins as .app bundles (ssh has no window server; `open` runs them
# in the logged-in GUI session), runs one tracking measurement per mode, then makes sure nothing is left running.
set -eu
cd "$(dirname "$0")"; O=${1:-/tmp/lane-m-proto}; rm -rf "$O"; mkdir -p "$O"
app() { # name -> $O/name.app
  A=$O/$1.app; mkdir -p $A/Contents/MacOS
  clang -fobjc-arc -O2 -framework Cocoa -framework QuartzCore -o $A/Contents/MacOS/$1 $1.m
  printf '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>%s</string><key>CFBundleIdentifier</key><string>works.backplane.lane-m.%s</string><key>CFBundlePackageType</key><string>APPL</string><key>NSHighResolutionCapable</key><true/></dict></plist>' $1 $1 > $A/Contents/Info.plist
  codesign --force --sign - $A >/dev/null 2>&1 || true
}
for p in host follower sampler; do app $p; done
waitline() { i=0; while [ ! -s "$1" ] && [ $i -lt 100 ]; do sleep 0.05; i=$((i+1)); done; head -1 "$1"; }
for mode in poll link; do
  open -n --stdout "$O/host.$mode.out" --stderr "$O/host.$mode.err" "$O/host.app" --args 6 600
  HID=$(waitline "$O/host.$mode.out")
  open -n --stdout "$O/fol.$mode.out" --stderr "$O/fol.$mode.err" "$O/follower.app" --args "$HID" 7.5 $mode
  FID=$(waitline "$O/fol.$mode.out")
  open -n -W --stdout "$O/samples.$mode.csv" --stderr "$O/sampler.$mode.err" "$O/sampler.app" --args "$HID" "$FID" 7.5
  sleep 0.5
  echo "$mode host=$HID follower=$FID $(tail -1 "$O/fol.$mode.out") $(grep reactivated "$O/host.$mode.out" || true) samples=$(wc -l < "$O/samples.$mode.csv")"
done
pkill -f "$O/" 2>/dev/null || true; sleep 0.3
pgrep -lf "$O/" || echo "nothing left running"
