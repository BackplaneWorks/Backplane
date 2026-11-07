#!/bin/sh
# Headless emulator on THIS desktop, own AVD, own capped user service, never on the screen.
# usage: scripts/android-emu.sh create | start | stop | status | adb ARGS...
# EMU_PORT (default 5584): each worktree that runs an emulator takes its own, or `adb` here reaches another thread's device.
# EMU_AVD (default bp-gpui-android): the AVD's name, in this worktree's own AVD folder.
# Only this instance is ever touched: it runs as its own user service (bp-emu-<port>), so it outlives the job that started it and one
# boot serves many runs; it gets its own netsimd instance (from its port), so starting or stopping it never restarts the netsimd other
# emulators use; stop ends this unit and this serial only. Nothing here stops netsimd or an adb server.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd); W=$ROOT/build/gpui/android
ANDROID_HOME=$HOME/Android-sdk-writable; export ANDROID_HOME ANDROID_SDK_ROOT=$ANDROID_HOME JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ANDROID_AVD_HOME=$W/avd
PORT=${EMU_PORT:-5584}; AVD=${EMU_AVD:-bp-gpui-android}; UNIT=bp-emu-$PORT; ADB="$ANDROID_HOME/platform-tools/adb -s emulator-$PORT"; IMG="system-images;android-34;google_apis;x86_64"
NETSIM=$(( PORT / 2 % 90 + 10 ))   # this emulator's own netsimd instance (consoles take even ports; the default instance, 0, is left to others)
booted() { [ "$($ADB shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; }
case "${1:-}" in
create) mkdir -p "$ANDROID_AVD_HOME"
  echo no | $ANDROID_HOME/cmdline-tools/latest/bin/avdmanager create avd -n "$AVD" -k "$IMG" -d pixel_6 --force ;;
start)
  if systemctl --user is-active -q "$UNIT"; then booted && { echo "running ($UNIT)"; exit 0; }; fi
  systemctl --user reset-failed "$UNIT" 2>/dev/null || true; mkdir -p "$W"
  systemd-run --user --unit "$UNIT" -q -p MemoryMax=8G -p MemorySwapMax=0 -p Nice=10 --setenv=ANDROID_AVD_HOME="$ANDROID_AVD_HOME" --setenv=ANDROID_HOME="$ANDROID_HOME" \
    --setenv=ANDROID_SDK_ROOT="$ANDROID_HOME" -p StandardOutput=truncate:"$W/emulator.log" -p StandardError=append:"$W/emulator.log" \
    $ANDROID_HOME/emulator/emulator -avd "$AVD" -port $PORT -no-window -no-audio -no-boot-anim -no-snapshot -gpu swiftshader_indirect -memory 2560 -cores 4 \
      -netsim-args --instance=$NETSIM
  echo "started $UNIT (netsimd instance $NETSIM)"
  sleep 3; systemctl --user is-active -q "$UNIT" || { echo "the emulator exited:"; head -20 "$W/emulator.log"; exit 3; }
  $ADB wait-for-device
  n=0; until booted; do n=$((n+1)); [ $n -gt 120 ] && { echo boot timeout; exit 2; }; sleep 2; done; echo booted ;;
status) systemctl --user is-active "$UNIT" || true; booted && echo booted || echo "not booted" ;;
stop) $ADB emu kill 2>/dev/null || true
  for i in 1 2 3 4 5 6 7 8; do systemctl --user is-active -q "$UNIT" || break; sleep 1; done
  systemctl --user stop "$UNIT" 2>/dev/null || true; systemctl --user reset-failed "$UNIT" 2>/dev/null || true; echo stopped ;;
adb) shift; exec $ADB "$@" ;;
esac
