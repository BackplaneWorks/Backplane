#!/bin/sh
# On the Mac: one cell of the composer measurement. usage: ios-keys-mac.sh KIND N_KB THREAD
# A draft of N KB is typed, then ten characters 150 ms apart (key cost), then the hub streams a turn into the open thread
# (screen updates with the draft present). Logs: key cost, stalls over 30 ms, BPPerf display stage.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
KIND=$1; N=$2; T=$3; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}
xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
if [ "$N" -gt 0 ]; then TXT=$(python3 -c "print(('lorem ipsum dolor sit amet ' * 400)[:${N}000], end='')"); ACTS="ob-skip;;@type=$TXT;;@times;;@times;;@keys=abcdefghij;;@times"; else ACTS="ob-skip;;@times;;@times;;@keys=abcdefghij;;@times"; fi
export EXTRAENV="BACKPLANE_STALL=1 BACKPLANE_COMPOSER=$KIND BACKPLANE_PERFORMANCE=1"
sh ~/bp-gpui-ios/ios-shot.sh $U keys-$KIND-$N 48 "$ACTS" $T > /dev/null
xcrun simctl spawn $U log show --last 90s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "key cost|stall|BPPerf" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' > ~/bp-gpui-ios/shots/keys-$KIND-$N.log
