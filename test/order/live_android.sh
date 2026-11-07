#!/bin/sh
# live_android.sh CLIENT_BIN HUB_BIN PYTHON : the Order card of a live buy thread on the Android data app, on this worktree's own headless emulator
# (EMU_PORT, default 5640; scripts/android-emu.sh start first, the debug APK built). live_e2e.sh makes the buy thread in the window and stops at
# the Order card (LIVE_NO_PAY=1); its hub and the order server are reached from the emulator at 127.0.0.1 through adb reverse. The app pairs
# through its deep link, opens the buy thread, Buy now is tapped and the browser it opens pays the Stripe double. Screenshots in $SHOTS.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); export EMU_PORT=${EMU_PORT:-5640}
E="sh $ROOT/scripts/android-emu.sh"; UI="python3 -I $ROOT/scripts/android-ui.py"; A="$E adb shell"; PKG=dev.backplane.mobile.gpui
OUT=${OUT:-$HOME/.cache/bp-tmp/order-live}; SHOTS=${SHOTS:-$HOME/.cache/commerce-live/shots/android}; mkdir -p "$SHOTS"; rm -f "$SHOTS"/*.png
fails=0; ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
shot() { $E adb exec-out screencap -p > "$SHOTS/$1.png"; echo "shot $SHOTS/$1.png"; }
want() { for i in $(seq 1 ${2:-20}); do $UI has "$1" >/dev/null 2>&1 && return 0; sleep 1; done; return 1; }
tap() { p=$($UI find "$1" 2>/dev/null) || return 1; $A input tap $p; sleep 1.5; }
# the rig: the window makes the buy thread and stops at a payable Order card
rm -f "$OUT/rig.json"; LIVE_KEEP=1 LIVE_NO_PAY=1 sh "$HERE/live_e2e.sh" "$1" "$2" "$3" > "$HOME/.cache/commerce-live/rig-android.log" 2>&1 & R=$!
for i in $(seq 1 300); do [ -f "$OUT/rig.json" ] && break; kill -0 $R 2>/dev/null || break; sleep 1; done
[ -f "$OUT/rig.json" ] || { echo "FAIL the rig did not come up"; cat "$HOME/.cache/commerce-live/rig-android.log"; exit 1; }
HUB=$(python3 -I -c "import json;print(json.load(open('$OUT/rig.json'))['hub'])"); FAB=$(python3 -I -c "import json;print(json.load(open('$OUT/rig.json'))['fab'])")
STRIPE=$(python3 -I -c "import json;print(json.load(open('$OUT/rig.json'))['stripe'])"); TOK=$(cat "$OUT/home/token"); fabpaid() { grep -c ':paid"' "$OUT/home/events.jsonl"; }
# first (adb root restarts adbd, which drops reverse ports): the emulator's Chrome without its first-run screens (a debug command line; the image allows adb root)
$E adb root >/dev/null 2>&1; sleep 3; $A "echo 'chrome --disable-fre --no-default-browser-check --no-first-run' > /data/local/tmp/chrome-command-line" >/dev/null 2>&1
$A am set-debug-app --persistent com.android.chrome >/dev/null 2>&1; $A am force-stop com.android.chrome >/dev/null 2>&1
$E adb wait-for-device
$E adb reverse tcp:$HUB tcp:$HUB >/dev/null; $E adb reverse tcp:$FAB tcp:$FAB >/dev/null; $E adb reverse tcp:$STRIPE tcp:$STRIPE >/dev/null
echo "reverse: $($E adb reverse --list | tr '\n' ' ')"
$E adb install -r "$ROOT/mobile/android/app/build/outputs/apk/debug/app-debug.apk" >/dev/null 2>&1 || $E adb install -r $(ls "$ROOT"/mobile/android/app/build/outputs/apk/debug/*.apk | head -1) >/dev/null
ENC=$(python3 -I -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "http://127.0.0.1:$HUB/#token=$TOK")
$A pm clear $PKG >/dev/null; $A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 6
# a link that pairs asks first (Pair only with a hub you know)
if want "Pair with this hub?" 10; then
  shot 0-pair-question; ok "a pairing link asks before it pairs"
  for i in 1 2 3; do
    # the pairing screen behind the question has its own Pair: the question's is the last one
    n=$($UI count "Pair" exact 2>/dev/null); b=$($UI bounds "Pair" exact $(( ${n:-1} - 1 )) 2>/dev/null); set -- $b
    echo "Pair buttons: $n, tapping the last at $b"; [ $# -eq 4 ] && $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 ))
    sleep 4; $UI has "Pair with this hub?" >/dev/null 2>&1 || break
  done
  $UI has "Pair with this hub?" >/dev/null 2>&1 && { echo "nodes with Pair:"; $E adb shell cat /sdcard/ui.xml | tr '>' '\n' | grep -i 'pair' | cut -c1-240; }
fi
want "lamp" 30 || { shot fail-paired; bad "the app is paired and lists the project"; }
shot 0-home
tapx() { p=$($UI find "$1" exact 2>/dev/null) || return 1; $A input tap $p; sleep 1.5; }
want "Buy: lamp" 8 || { tapx "lamp" || tapx "LAMP"; }
if want "Buy: lamp" 15; then tapx "Buy: lamp" || tap "Buy: lamp"; ok "the buy thread is listed"; else shot fail-list; bad "the buy thread is listed"; fi
if want "ORDER · lamp" 30; then ok "the Order card is on the buy thread"; else bad "the Order card is on the buy thread"; fi
want "Includes import VAT" 5 && ok "the card says what the total covers about customs" || bad "the customs line"
want "Buy now" 5 && ok "Buy now is on the card" || bad "Buy now is on the card"
shot 1-order-card
before=$(fabpaid); tap "Buy now · " || tap "Buy now" || bad "Buy now can be tapped"
sleep 6; want "Use without an account" 3 && { tapx "Use without an account"; sleep 4; }
want "No thanks" 3 && { tapx "No thanks"; sleep 3; }
sleep 4; shot 2-browser
top=$($A dumpsys activity activities 2>/dev/null | grep -m1 -E 'topResumedActivity|mResumedActivity' | tr -d '\r')
case "$top" in *$PKG*) bad "Buy now opened the browser ($top)";; *) ok "Buy now opened the browser ($(echo "$top" | grep -oE '[a-z][a-z0-9_.]+/[A-Za-z0-9_.]+' | head -1))";; esac
for i in $(seq 1 90); do [ "$(fabpaid)" -gt "$before" ] && break; sleep 2; done
[ "$(fabpaid)" -gt "$before" ] && ok "the browser's test payment came back to the hub as Paid" || bad "the payment came back as Paid"
$A "am start -n $PKG/dev.backplane.mobile.DataActivity" >/dev/null; sleep 4; want "Paid" 20 && ok "the phone's buy thread shows Paid" || bad "the phone shows Paid"
shot 3-paid
$E adb reverse --remove-all >/dev/null; touch "$OUT/stop"; wait $R 2>/dev/null
grep -E '^(PASS|FAIL)' "$HOME/.cache/commerce-live/rig-android.log" | sed 's/^/window: /'
echo "$fails failed"; exit $fails
