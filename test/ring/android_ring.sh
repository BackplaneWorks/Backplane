#!/bin/sh
# Settings → Ring on the Android app (emulator), against test/ring/rig.py's temp hub: pair, open Settings, Ring: Set up, Copy both (the app says Copied),
# the Pebble app's test event (rig.py test) and Check again shows when it came, a recording (rig.py record) listed with Play and What happened.
# usage: EMU_PORT=<port> test/ring/android_ring.sh HUB_BIN VOICE_BIN LOCAL_DIR SAMPLE.m4a   (APK installed; emulator booted). Shots: build/mobile/shots/ring-*.png
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/android-emu.sh; UI="python3 $ROOT/scripts/android-ui.py"; S=$ROOT/build/mobile/shots; mkdir -p $S
A="$E adb shell"; PKG=dev.backplane.mobile.gpui; . "$ROOT/scripts/lib-bptmp.sh"; R=$BP_TMP/ring-android; rm -rf $R
RIG="python3 -I $ROOT/test/ring/rig.py"; fails=0; HUBBIN=$1; VOICEBIN=$2; LOCALDIR=$3; SAMPLE=$4
shot() { $E adb exec-out screencap -p > $S/ring-$1.png; }
has() { $UI has "$@" 2>/dev/null; }
ok() { echo "PASS $1"; }
bad() { echo "FAIL $1"; fails=$((fails+1)); shot "fail-$(echo $1 | tr -c 'a-zA-Z0-9' _)"; }
want() { what=$1; shift; for i in $(seq 1 ${WAIT:-12}); do has "$@" && { ok "$what"; return 0; }; sleep 1; done; bad "$what ('$1' not shown)"; return 1; }
# scroll until TEXT is on screen (down, then up), then tap it
find_on() { for i in $(seq 1 14); do p=$($UI find "$@" 2>/dev/null) && { echo $p; return 0; }; $A input swipe 540 1700 540 900 300; sleep 0.7; done; return 1; }
tapt() { p=$(find_on "$@") || { bad "no '$1' to tap"; return 1; }; $A input tap $p; sleep 1.5; }
LINKHOST=10.0.2.2 $RIG up "$HUBBIN" "$VOICEBIN" "$LOCALDIR" $R || exit 1; trap '$RIG down $R >/dev/null' EXIT
LINK=$(sed -n 's/^LINK=//p' $R/env); ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "$LINK")
$A pm clear $PKG >/dev/null; $A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 6
if has "Pair with this hub?"; then set -- $($UI bounds "Pair" exact $(( $($UI count "Pair" exact) - 1 ))); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 5; fi
want "paired: the home list" "Settings" exact; shot home
tapt "Settings" exact
find_on "RING" exact >/dev/null && ok "Settings shows the Ring page" || bad "no Ring page in Settings"; shot page
tapt "Set up" exact && sleep 3
WAIT=20 want "Set up made the webhook: the URL to paste" "/hook/h" && shot set-up
tapt "Copy both" exact && { WAIT=4 want "Copy both: the app says Copied" "Copied"; }
has "None yet." && ok "no test event yet" || bad "the test row says none yet"
echo "rig test: $($RIG test $R)"
tapt "Check again" exact; WAIT=10 want "the test event's time shows after Check again" "Backplane hears this webhook." && shot test-arrived
echo "rig record: $($RIG record $R "$SAMPLE" rec-android-1)"
for i in $(seq 1 40); do sleep 2; tapt "Check again" exact >/dev/null 2>&1; has "given to Pebble" && break; done
WAIT=4 want "the recording is listed, heard and given to Pebble" "given to Pebble" && shot recording
p=$(find_on "Play" exact) && ok "the recording has Play" || bad "no Play"
p=$(find_on "What happened" exact) && ok "the recording has What happened" || bad "no What happened"
tapt "What happened" exact && sleep 2 && { WAIT=8 want "What happened opens Pebble's thread" "voice note from your Pebble ring" && shot what-happened; }
echo "$( [ $fails = 0 ] && echo OK || echo FAILED ): $fails failed"; exit $fails
