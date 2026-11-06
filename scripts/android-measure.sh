#!/bin/sh
# Emulator numbers (NOT device timing). usage: android-measure.sh
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd); E="$ROOT/scripts/android-emu.sh"; R=$ROOT/build/gpui/results/android; PK=dev.backplane.mobile.gpui
pss() { $E adb shell dumpsys meminfo $PK | awk '/TOTAL PSS:/{print $3}'; }
out=$R/measure.txt; : > $out
echo "emulator x86_64 api34 google_apis, 4 cores, swiftshader; desktop host; commit $(git -C $ROOT rev-parse --short HEAD); engine: $($E adb logcat -d | grep -o 'engine [A-Za-z0-9]* in' | tail -1)" >> $out
echo "cold launch (am start -W TotalTime ms, force-stop between), 7 samples:" >> $out
for i in 1 2 3 4 5 6 7; do $E adb shell am force-stop $PK; sleep 3; $E adb shell am start -W -n $PK/dev.backplane.mobile.MainActivity | awk '/TotalTime/{print $2}' >> $out; sleep 4; done
sleep 20; echo "PSS KB idle on thread list (after 30 s): $(pss)" >> $out
echo "engine ready ms (logcat): $($E adb logcat -d | grep 'engine ready' | sed 's/.*ready //' | tr '\n' ' ')" >> $out
# open thread i, back, repeated; 5 distinct threads x 20 cycles each = 100 opens
n=0
for cyc in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  for y in 746 893 1041 1188 1336; do $E adb shell input tap 450 $y; sleep 1; $E adb shell input keyevent 4; sleep 1; n=$((n+1)); done
  [ $((cyc % 4)) = 0 ] && echo "after $n opens: PSS KB $(pss)" >> $out
done
$E adb shell dumpsys meminfo $PK | grep -E "Java Heap|Native Heap|TOTAL PSS|Views|Activities" >> $out
cat $out
