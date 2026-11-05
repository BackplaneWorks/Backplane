#!/bin/sh
# Boat: JS hub from the branch's current src (ios-hub-boat.sh recipe); artifact copied from THIS job's folder by id.
# Result: build/gpui/hub/android/backplane-serve
cd "$(dirname "$0")/../.." || exit 1
LOG=$(mktemp)
scripts/boat-job.sh --no-setup-bend --size large --timeout 5400 --artifact build/backplane-serve -- sh scripts/gpui/ios-hub-boat.sh 2>&1 | tee "$LOG"
ID=$(grep -o "job j[0-9]*-[0-9]*" "$LOG" | head -1 | cut -d" " -f2); rm -f "$LOG"
[ -n "$ID" ] || { echo "no job id in boat-job output"; exit 1; }
J="${XDG_STATE_HOME:-$HOME/.local/state}/backplane/boat/jobs/$ID"
F=$(find "$J" -name backplane-serve -type f | head -1)
[ -n "$F" ] || { echo "no artifact in $J"; exit 1; }
mkdir -p build/gpui/hub/android && cp "$F" build/gpui/hub/android/backplane-serve && sha256sum build/gpui/hub/android/backplane-serve
git rev-parse HEAD
