#!/bin/sh
# On the Mac: start-up phase marks (Backplane boot <name> <ms since process start>) and BPPerf stages. usage: ios-boot-mac.sh THREAD
# fresh = uninstall first (no cache); relaunch = same install, cache and draft present. Three of each.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
T=$1; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; O=~/bp-gpui-ios/shots/boot.log; : > $O
xcrun simctl boot $U 2>/dev/null
export EXTRAENV="BACKPLANE_PERFORMANCE=1"
one() { # label
  sh ~/bp-gpui-ios/ios-run.sh $U "$2" $T >/dev/null; sleep 22
  echo "== $1" >> $O
  xcrun simctl spawn $U log show --last 30s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "Backplane boot|BPPerf" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' >> $O
}
xcrun simctl uninstall $U dev.backplane.gpui
one fresh1 "ob-skip"
for i in 1 2 3; do one relaunch$i "ob-skip"; done
xcrun simctl uninstall $U dev.backplane.gpui; one fresh2 "ob-skip"
xcrun simctl uninstall $U dev.backplane.gpui; one fresh3 "ob-skip"
xcrun simctl shutdown $U 2>/dev/null
