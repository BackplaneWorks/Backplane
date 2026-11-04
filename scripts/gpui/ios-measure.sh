#!/bin/sh
# On the Mac. ios-measure.sh UDID TAG CYCLES "THREADIDS..."  (env JIT=0 for no-JIT JSC, HUBURL/TOKFILE)
# Writes ~/bp-gpui-ios/results/TAG.{cold,csv}: cold launch BPPerf startup records, then a footprint sample
# every 10 s while ACTS visit every thread (select, then back) CYCLES times.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=$1; TAG=$2; CYC=$3; IDS=$4
O=~/bp-gpui-ios/results; mkdir -p $O
KEY=${KEY:-desktop.tail9e0cf2.ts.net:3811}
export BACKPLANE_PERF=1 SIMCTL_CHILD_BACKPLANE_PERFORMANCE=1
pidof() { xcrun simctl spawn $U launchctl list | grep dev.backplane.gpui | awk '{print $1}'; }
fp() { footprint "$1" 2>/dev/null | awk '/phys_footprint:/ {print $2,$3} /phys_footprint_peak:/ {print "peak",$2,$3}' | tr '\n' ' '; }
# cold: kill, launch, read startup records
xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1; sleep 2
T0=$(date +%s)
sh ~/bp-gpui-ios/ios-run.sh $U 'ob-skip' >/dev/null
sleep 25
xcrun simctl spawn $U log show --last 40s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep BPPerf | sed 's/.*BPPerf//' > $O/$TAG.cold
P=$(pidof); echo "idle $(fp $P)" > $O/$TAG.idle
# visits
ACTS=""
c=0; while [ $c -lt $CYC ]; do for id in $IDS; do ACTS="$ACTS;;select=$KEY|$id;;!@back"; done; c=$((c+1)); done
ACTS=${ACTS#;;}
xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1; sleep 2
sh ~/bp-gpui-ios/ios-run.sh $U "ob-skip;;$ACTS" >/dev/null
sleep 8; P=$(pidof); echo "t,footprint,peak" > $O/$TAG.csv
N=$(echo $IDS | wc -w); TOTAL=$((CYC * N * 2 + 20)); t=0
while [ $t -lt $TOTAL ]; do echo "$t,$(fp $P)" >> $O/$TAG.csv; sleep 10; t=$((t+10)); done
echo "end $(fp $P)" >> $O/$TAG.csv
