#!/bin/sh
# Uploaded and run by boat-job.sh: the bridge steps of scripts/build-mobile.sh (no APK), in an 8 GiB, zero-swap scope.
set -eu
[ "${BACKPLANE_HEAVY_BOAT_SANDBOX:-}" = 1 ] || { echo 'bridge builds run only on Boat' >&2; exit 75; }
CAP=8589934592
CHECK='g=$(cut -d: -f3 /proc/self/cgroup); b=/sys/fs/cgroup$g; mx=$(cat "$b/memory.max"); sw=$(cat "$b/memory.swap.max"); case "$mx" in ""|*[!0-9]*) exit 1;; esac; [ "$mx" -gt 0 ] && [ "$mx" -le 8589934592 ] && [ "$sw" = 0 ]'
if ! sh -c "$CHECK"; then
 [ "${GPUI_CAP_TRIED:-}" != 1 ] || { echo "kernel memory cap unavailable; refusing" >&2; exit 75; }
 export GPUI_CAP_TRIED=1
 exec systemd-run --user --scope -q -p MemoryAccounting=yes -p MemoryMax=$CAP -p MemorySwapMax=0 -- sh "$0" "$@"
fi
A=build/gpui-bridge-artifacts; rm -rf $A; mkdir -p $A
sh -c 'g=$(cut -d: -f3 /proc/self/cgroup); b=/sys/fs/cgroup$g; cat "$b/memory.max" "$b/memory.swap.max"' > $A/cap.txt
bend --version > $A/bend-version.txt 2>&1 || true
# the heap would otherwise size itself to the machine (AGENTS memory notes)
export BUN_JSC_forceRAMSize=4294967296 BEND_NO_TELEMETRY=1
rm -rf build/mobile-web
/usr/bin/time -v -o $A/emit.time bend src/mobile/index.html -o build/mobile-web > $A/emit.log 2>&1 || { tail -30 $A/emit.log; exit 1; }
mkdir -p mobile/build/assets
cat build/mobile-web/chunk-*.js > mobile/build/assets/bridge.js
python3 scripts/bridge-native.py mobile/build/assets/bridge.js
python3 scripts/js-unmarshal.py hubs.Hubs mobile/build/assets/bridge.js
printf '\nglobalThis.BackplaneStateKey = "%s";\n' "$(python3 scripts/state-key.py)" >> mobile/build/assets/bridge.js
cp mobile/build/assets/bridge.js $A/bridge.js
sha256sum $A/bridge.js > $A/bridge.sha256; stat -c %s $A/bridge.js > $A/bridge.bytes
grep -E "Maximum resident|Elapsed" $A/emit.time > $A/emit-summary.txt; cat $A/emit-summary.txt $A/cap.txt
