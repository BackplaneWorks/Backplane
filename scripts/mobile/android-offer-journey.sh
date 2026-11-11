#!/bin/sh
# The home hub's offer and failover (docs/pairing.md, Home hub) on this worktree's own headless emulator, against two stand-in hubs (test/mobile/offer_stub.py):
# "desk" offers "boat" with the phone's own token. One screenshot at each moment, what each stand-in was asked, and the app's BPPAIR lines.
# usage: scripts/mobile/android-offer-journey.sh APK [OUT]      (EMU_PORT default 5590: this worktree's emulator; stopped at the end)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); APK=$1; OUT=${2:-${BP_TMP:-$HOME/.cache/bp-tmp}/android-offer}
export EMU_PORT=${EMU_PORT:-5590}; E="$ROOT/scripts/android-emu.sh"; U="python3 $ROOT/scripts/android-ui.py"; PKG=dev.backplane.mobile.gpui
T=0123456789abcdef0123456789abcdef; HERE=http://127.0.0.1
rm -rf "$OUT"; mkdir -p "$OUT"
[ -d "$ROOT/build/gpui/android/avd" ] || sh "$E" create >/dev/null
sh "$E" start || exit 1
DP=; BP=
desk() { python3 "$ROOT/test/mobile/offer_stub.py" 3831 desk $T http://10.0.2.2:3832 same > "$OUT/desk.log" 2>&1 & DP=$!; sleep 1; }
python3 "$ROOT/test/mobile/offer_stub.py" 3832 boat $T > "$OUT/boat.log" 2>&1 & BP=$!
desk
stop() { kill $DP $BP 2>/dev/null; sh "$E" stop >/dev/null 2>&1; }
trap stop EXIT
shot() { sh "$E" adb exec-out screencap -p > "$OUT/$1.png"; echo "shot $1"; }
seen() { echo "-- $1 asked of $2:"; curl -s -m3 "$3/__seen" | python3 -c 'import json,sys; [print("  ", json.dumps(x)) for x in json.load(sys.stdin)]' 2>/dev/null || echo "   (down)"; }
sh "$E" adb uninstall $PKG >/dev/null 2>&1; sh "$E" adb install "$APK" | tail -1
sh "$E" adb shell pm grant $PKG android.permission.POST_NOTIFICATIONS
sh "$E" adb logcat -c
# 1. paired with desk alone, as a scanned code is: the offer is seen, boat is added silently with the same token, desk stays in use
L=$(python3 -c 'import urllib.parse;print(urllib.parse.quote("http://10.0.2.2:3831/pair#invite="+"ab"*16,safe=""))')
sh "$E" adb shell am start -W -a android.intent.action.VIEW -d "'bpgpui://pair?url=$L'" $PKG >/dev/null
# the question's Pair is the last one (the pairing form under it has its own)
for i in $(seq 1 20); do n=$($U count Pair exact 2>/dev/null); [ "${n:-0}" -ge 2 ] && { set -- $($U bounds Pair exact $((n-1))); sh "$E" adb shell input tap $((($1+$3)/2)) $((($2+$4)/2)); break; }; sleep 1; done
for i in $(seq 1 20); do xy=$($U find Skip exact 2>/dev/null) && { sh "$E" adb shell input tap $xy; break; }; sleep 1; done
sleep 12; shot 1-on-desk-boat-added
seen 1 desk $HERE:3831; seen 1 boat $HERE:3832
# 2. desk stops: after about 20 s the phone is on boat
kill $DP; sleep 35; shot 2-desk-down-on-boat
seen 2 boat $HERE:3832
# 3. desk answers again: back on desk
desk; sleep 25; shot 3-desk-back
seen 3 desk $HERE:3831
# 4. a start with desk asleep: the saved list and home; boat within about 20 s
kill $DP; sh "$E" adb shell am force-stop $PKG; sh "$E" adb shell am start -n $PKG/dev.backplane.mobile.DataActivity >/dev/null; sleep 40; shot 4-start-desk-asleep-on-boat
sh "$E" adb logcat -d -s BPPAIR:I | grep -v 'token=' > "$OUT/bppair.log"; echo "-- the app said:"; cat "$OUT/bppair.log"
echo "shots in $OUT"
