#!/bin/sh
# Headless emulator on THIS desktop, own AVD, own capped scope, never on the screen.
# usage: scripts/android-emu.sh create | start | stop | adb ARGS...
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd); W=$ROOT/build/gpui/android
ANDROID_HOME=$HOME/Android-sdk-writable; export ANDROID_HOME ANDROID_SDK_ROOT=$ANDROID_HOME JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ANDROID_AVD_HOME=$W/avd
ADB="$ANDROID_HOME/platform-tools/adb -s emulator-5584"; IMG="system-images;android-34;google_apis;x86_64"
case "${1:-}" in
create) mkdir -p "$ANDROID_AVD_HOME"
  echo no | $ANDROID_HOME/cmdline-tools/latest/bin/avdmanager create avd -n bp-gpui-android -k "$IMG" -d pixel_6 --force ;;
start)
  [ -f "$W/emu.pid" ] && kill -0 "$(cat "$W/emu.pid")" 2>/dev/null && { echo "running $(cat "$W/emu.pid")"; exit 1; }
  nohup systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 nice -n 10 \
    $ANDROID_HOME/emulator/emulator -avd bp-gpui-android -port 5584 -no-window -no-audio -no-boot-anim -no-snapshot -gpu swiftshader_indirect -memory 2560 -cores 4 \
    > "$W/emulator.log" 2>&1 &
  echo $! > "$W/emu.pid"; echo "scope launcher pid $(cat "$W/emu.pid")"
  $ADB wait-for-device
  n=0; until [ "$($ADB shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; do n=$((n+1)); [ $n -gt 120 ] && { echo boot timeout; exit 2; }; sleep 2; done; echo booted ;;
stop) $ADB emu kill 2>/dev/null || true; sleep 4
  p=$(cat "$W/emu.pid" 2>/dev/null || true); [ -n "$p" ] && kill -0 "$p" 2>/dev/null && kill "$p" || true
  rm -f "$W/emu.pid"; echo stopped ;;
adb) shift; exec $ADB "$@" ;;
esac
