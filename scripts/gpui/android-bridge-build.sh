#!/bin/sh
# Boat: bridge.js from the branch (focused checks first), then the APK from it. Result: build/gpui/bridge/bridge.js, apk per android-build.sh
cd "$(dirname "$0")/../.." || exit 1
LOG=$(mktemp)
scripts/boat-job.sh --size large --exclude "native/host/target/**" --exclude "stage/**" --exclude "stage-edge/**" --timeout 3000 --artifact build/gpui-bridge-artifacts -- sh scripts/gpui/android-bridge-boat.sh test/mbots_test.bend 2>&1 | tee "$LOG"
ID=$(grep -o "job j[0-9]*-[0-9]*" "$LOG" | head -1 | cut -d" " -f2); rm -f "$LOG"
[ -n "$ID" ] || { echo "no job id"; exit 1; }
J="${XDG_STATE_HOME:-$HOME/.local/state}/backplane/boat/jobs/$ID"
F=$(find "$J" -name bridge.js -type f | head -1)
[ -n "$F" ] || { echo "no bridge.js in $J"; exit 1; }
mkdir -p build/gpui/bridge
[ -f build/gpui/bridge/bridge.js ] && cp build/gpui/bridge/bridge.js build/gpui/bridge/bridge.js.prev
cp "$F" build/gpui/bridge/bridge.js
S=$(sha256sum build/gpui/bridge/bridge.js | cut -c1-64)
printf 'bridge.js %s bytes sha256 %s commit %s job %s\n' "$(stat -c %s build/gpui/bridge/bridge.js)" "$S" "$(git rev-parse --short HEAD)" "$ID" >> build/gpui/bridge/PROVENANCE.txt
echo "BRIDGE $S"
exec scripts/gpui/android-build.sh
