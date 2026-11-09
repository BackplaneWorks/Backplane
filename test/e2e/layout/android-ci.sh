#!/bin/bash
# The Android size matrix inside the emulator runner (.github/workflows/e2e-sizes.yml job `android`): the emulator is up; hub and APK come from the `android-build` job.
#   android-ci.sh SIZE ORIENT OUT       SIZE = quick | small | regular | large | tablet, ORIENT = both | portrait | landscape
set -u
SIZE=$1; ORIENT=$2; OUT=$3
cd "$(dirname "$0")/../../.."
adb wait-for-device; until [ "$(adb shell getprop sys.boot_completed | tr -d '\r')" = 1 ]; do sleep 2; done
adb shell settings put global window_animation_scale 0; adb shell settings put global transition_animation_scale 0; adb shell settings put global animator_duration_scale 0
chmod +x sizes-build/backplane-hub
args=(--out "$OUT" --serial emulator-5554 --hub sizes-build/backplane-hub --apk sizes-build/app-debug.apk --pages 3)
if [ "$SIZE" = quick ]; then args+=(--quick); else args+=(--sizes "$SIZE"); [ "$ORIENT" = both ] || args+=(--orient "$ORIENT"); fi
python3 -W ignore test/e2e/layout/android.py "${args[@]}"
