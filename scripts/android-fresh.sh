#!/bin/sh
# Fresh app state paired with a hub: android-fresh.sh APK PORT TOKENFILE [dark|light]  (uninstall keeps keys honest across Boat/desktop signing)
ROOT=$(cd "$(dirname "$0")/.." && pwd); E=$ROOT/scripts/android-emu.sh; U="python3 $ROOT/scripts/android-ui.py"
$E adb uninstall dev.backplane.mobile.gpui >/dev/null 2>&1; $E adb install "$1" | tail -1
$E adb shell cmd uimode night $([ "${4:-light}" = dark ] && echo yes || echo no) >/dev/null
$E adb shell pm grant dev.backplane.mobile.gpui android.permission.POST_NOTIFICATIONS
PORT=$2 TOKENFILE=$3 $ROOT/scripts/android-pair.sh >/dev/null
for i in $(seq 1 40); do $U has Skip exact && break; done; xy=$($U find Skip exact) && $E adb shell input tap $xy; sleep 2
