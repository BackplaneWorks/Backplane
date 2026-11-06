#!/bin/sh
# On the Mac: open threads one after another (three first opens, then twelve cached ones, alternating) and keep the app's BPPerf records
# (navigation = op 6; display = stage 8: outcome 3 is the cached page on screen, 0 the hub's fresh page).
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3850/ TOKFILE=$HOME/bp-gpui-ios/e2e-token
U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; APP=${APP:-$HOME/bp-gpui-ios/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app}
set -- $(cat $HOME/bp-gpui-ios/e2e-threads); A=$1; B=$2; C=$3
K=backplane.tail9e0cf2.ts.net:3850
xcrun simctl boot $U 2>/dev/null; xcrun simctl bootstatus $U -b >/dev/null 2>&1
xcrun simctl terminate $U dev.backplane.gpui 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui; xcrun simctl install $U "$APP" || exit 1
ACTS="ob-skip"; for t in $A $B $C; do ACTS="$ACTS;;select=$K|$t;;@back"; done
n=0; while [ $n -lt 4 ]; do for t in $A $B $C; do ACTS="$ACTS;;select=$K|$t;;@back"; done; n=$((n+1)); done
export EXTRAENV="BACKPLANE_PERFORMANCE=1"
sh $HOME/bp-gpui-ios/ios-run.sh $U "$ACTS" >/dev/null; sleep 75
xcrun simctl spawn $U log show --last 90s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "BPPerf" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' | cut -c1-2000 > $HOME/bp-gpui-ios/shots/switch.txt
xcrun simctl io $U screenshot $HOME/bp-gpui-ios/shots/switch.png 2>&1 | tail -1
xcrun simctl shutdown $U 2>/dev/null
