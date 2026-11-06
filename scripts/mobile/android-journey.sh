#!/bin/sh
# Drives the Android data app on the local emulator against scripts/mobile/data-hub.sh's hub; screenshots to build/mobile/shots.
# usage: scripts/mobile/android-journey.sh   (app installed, hub running; pairs by link, opens a thread, sends, waits for the stand-in's stream)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/gpui/android-emu.sh; S=$ROOT/build/mobile/shots; mkdir -p $S
A="$E adb shell"; shot() { $E adb exec-out screencap -p > $S/$1.png; }
tap() { $A input tap "$1" "$2"; sleep 1; }
LINK=$($ROOT/scripts/mobile/data-hub.sh link)
ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "$LINK")
$A am force-stop dev.backplane.mobile.gpui
$A "am start -n dev.backplane.mobile.gpui/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 5; shot j1-home
tap 480 440; sleep 2; shot j2-thread
tap 450 2250; $A input text "hello%sfrom%sthe%sphone"; sleep 1; shot j3-typed
python3 $ROOT/scripts/gpui/android-ui.py find "Send" >/dev/null && tap $(python3 $ROOT/scripts/gpui/android-ui.py find "Send")
sleep 1; shot j4-sending; sleep 3; shot j5-streaming; sleep 8; shot j6-done
