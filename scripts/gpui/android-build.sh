#!/bin/sh
# Build the GPUI experiment's debug APK on BOAT (never on this desktop). Run it through work_run with lock gpui-boat-build.
# usage: scripts/gpui/android-build.sh [default|nosidebar]   -> build/gpui/android/apk-<variant>.apk and results/android/builds.jsonl
# bridge: build/gpui/bridge/bridge.js (itself built on Boat: scripts/gpui/android-bridge-boat.sh), uploaded with the job.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); V=${1:-default}; cd "$ROOT"
[ -f build/gpui/bridge/bridge.js ] || { echo "no build/gpui/bridge/bridge.js"; exit 1; }
before=$(ls -d "${XDG_STATE_HOME:-$HOME/.local/state}"/backplane/boat/jobs/*/ 2>/dev/null | sort | tail -1)
ARG=""; [ "$V" = nosidebar ] && ARG=nosidebar
scripts/boat-job.sh --setup-bend --timeout 2400 --include-file build/gpui/bridge/bridge.js --artifact build/gpui-apk-artifacts -- sh scripts/gpui/android-build-boat.sh $ARG
J=$(ls -dt "${XDG_STATE_HOME:-$HOME/.local/state}"/backplane/boat/jobs/*/ | head -1)
mkdir -p build/gpui/android; cp "$J"artifacts/*/app-debug.apk "build/gpui/android/apk-$V.apk"
echo "$(cat "$J"artifacts/*/timing.json | head -1 | sed 's/}$/,"commit":"'"$(git rev-parse --short HEAD)"'"}/')" | tee -a build/gpui/results/android/builds.jsonl
