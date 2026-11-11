#!/bin/sh
# The home hub's offer and failover on this worktree's emulator against two real temp hubs after a home move (test/e2e/homemove/hold_phone.py):
# the phone pairs with A (the desktop) by its own device token, adds B with it silently, goes to B when A stops and back when A is up again.
# usage: scripts/mobile/android-homemove-journey.sh HUB_BIN APK [OUT]      (EMU_PORT default 5590; the hubs' ports reach the emulator by adb reverse)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); HUB=$1; APK=$2; OUT=${3:-${BP_TMP:-$HOME/.cache/bp-tmp}/android-homemove}
export EMU_PORT=${EMU_PORT:-5590}; E="$ROOT/scripts/android-emu.sh"; U="python3 $ROOT/scripts/android-ui.py"; PKG=dev.backplane.mobile.gpui
rm -rf "$OUT"; mkdir -p "$OUT/work"; W=$OUT/work
python3 "$ROOT/test/e2e/homemove/hold_phone.py" "$HUB" "$W" > "$OUT/hubs.log" 2>&1 & HP=$!
stop() { touch "$W/quit"; sleep 3; kill $HP 2>/dev/null; sh "$E" stop >/dev/null 2>&1; }
trap stop EXIT
i=0; until [ -s "$W/phone" ]; do i=$((i+1)); [ $i -gt 300 ] && { echo "the hubs never got ready"; cat "$OUT/hubs.log"; exit 1; }; kill -0 $HP 2>/dev/null || { cat "$OUT/hubs.log"; exit 1; }; sleep 1; done
grep '^move:' "$OUT/hubs.log"
[ -d "$ROOT/build/gpui/android/avd" ] || sh "$E" create >/dev/null
sh "$E" start >/dev/null || exit 1
sh "$E" adb reverse tcp:3861 tcp:3861 >/dev/null; sh "$E" adb reverse tcp:3862 tcp:3862 >/dev/null
shot() { sh "$E" adb exec-out screencap -p > "$OUT/$1.png"; echo "shot $1"; }
said() { sh "$E" adb logcat -d -s BPPAIR:I | grep -v 'token=' | sed -n 's/.*BPPAIR *: //p'; }
sh "$E" adb uninstall $PKG >/dev/null 2>&1; sh "$E" adb install "$APK" | tail -1
sh "$E" adb shell pm grant $PKG android.permission.POST_NOTIFICATIONS; sh "$E" adb logcat -c
L=$(python3 -c 'import sys,urllib.parse;print(urllib.parse.quote(open(sys.argv[1]).read().strip(),safe=""))' "$W/phone")
sh "$E" adb shell am start -W -a android.intent.action.VIEW -d "'bpgpui://pair?url=$L'" $PKG >/dev/null
for i in $(seq 1 20); do n=$($U count Pair exact 2>/dev/null); [ "${n:-0}" -ge 2 ] && { set -- $($U bounds Pair exact $((n-1))); sh "$E" adb shell input tap $((($1+$3)/2)) $((($2+$4)/2)); break; }; sleep 1; done
for i in $(seq 1 20); do xy=$($U find Skip exact 2>/dev/null) && { sh "$E" adb shell input tap $xy; break; }; sleep 1; done
sleep 15; shot 1-on-A-B-added
touch "$W/stopA"; sleep 40; shot 2-A-stopped-on-B
touch "$W/startA"; sleep 35; shot 3-A-back
said > "$OUT/bppair.log"; echo "-- the app said:"; cat "$OUT/bppair.log"
echo "-- hub.offer.done on A (its log):"; grep -h 'hub offer' "$W"/A.log | head -3
echo "shots in $OUT"
