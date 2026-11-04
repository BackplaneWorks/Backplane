#!/bin/sh
# Runs ON the Mac while holding the mac-heavy lock.
# usage: android-emu.sh start | stop | adb ARGS...
T=$HOME/bp-gpui-android/toolchain; D=$HOME/bp-gpui-android
export JAVA_HOME=$T/jdk/Contents/Home ANDROID_HOME=$T/sdk ANDROID_SDK_ROOT=$T/sdk ANDROID_AVD_HOME=$T/avd
ADB=$T/sdk/platform-tools/adb
case "$1" in
start)
  [ -f $D/emu.pid ] && kill -0 "$(cat $D/emu.pid)" 2>/dev/null && { echo "already running pid $(cat $D/emu.pid)"; exit 1; }
  nohup $T/sdk/emulator/emulator -avd bp-gpui-android -port 5584 -no-window -no-audio -no-boot-anim -no-snapshot -gpu swiftshader_indirect -memory 3072 > $D/logs/emulator.log 2>&1 &
  echo $! > $D/emu.pid; echo "emulator pid $(cat $D/emu.pid)"
  $ADB -s emulator-5584 wait-for-device
  n=0; until [ "$($ADB -s emulator-5584 shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; do n=$((n+1)); [ $n -gt 150 ] && { echo boot timeout; exit 2; }; sleep 2; done
  echo booted ;;
stop)
  $ADB -s emulator-5584 emu kill 2>/dev/null; sleep 5
  p=$(cat $D/emu.pid 2>/dev/null); [ -n "$p" ] && kill -0 $p 2>/dev/null && kill $p
  rm -f $D/emu.pid; echo stopped ;;
adb) shift; exec $ADB -s emulator-5584 "$@" ;;
esac
