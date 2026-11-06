#!/bin/sh
# A real two-finger pinch on the emulator (adb's `input` has one finger only): multi-touch events written with sendevent to the emulator's touch screen.
# usage: scripts/mobile/android-pinch.sh DEVICE out|in [CX CY]    (DEVICE like /dev/input/event12; coordinates in screen pixels, default the middle of the board view)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/android-emu.sh
DEV=$1; DIR=${2:-out}; CX=${3:-540}; CY=${4:-1100}
ax() { echo $(( $1 * 32767 / 1080 )); }; ay() { echo $(( $1 * 32767 / 2400 )); }
{
  S() { echo "sendevent $DEV 3 47 $1"; }   # slot
  T() { echo "sendevent $DEV 3 57 $1"; }   # tracking id
  X() { echo "sendevent $DEV 3 53 $(ax $1)"; }; Y() { echo "sendevent $DEV 3 54 $(ay $1)"; }
  P() { echo "sendevent $DEV 3 58 400"; }
  R() { echo "sendevent $DEV 0 0 0"; }
  if [ "$DIR" = out ]; then a=60; b=330; else a=330; b=60; fi
  S 0; T 101; X $((CX - a)); Y $CY; P; S 1; T 102; X $((CX + a)); Y $CY; P; echo "sendevent $DEV 1 330 1"; R
  i=1; while [ $i -le 10 ]; do d=$(( a + (b - a) * i / 10 )); S 0; X $((CX - d)); Y $CY; S 1; X $((CX + d)); Y $CY; R; echo "sleep 0.03"; i=$((i + 1)); done
  S 0; T -1; S 1; T -1; echo "sendevent $DEV 1 330 0"; R
} > /tmp/pinch.sh
$E adb push /tmp/pinch.sh /data/local/tmp/pinch.sh >/dev/null
$E adb shell sh /data/local/tmp/pinch.sh
