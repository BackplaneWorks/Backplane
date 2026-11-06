#!/bin/sh
# On the Mac: start-up from PROCESS START (Backplane boot <name> <ms since the process was created>), one launch per run, no thread
# selected (a select starts a navigation that replaces the start-up timer, so a run could end without a modelReady).
# usage: ios-boot-mac.sh [RUNS]   env: APP (default: the Debug build)  fresh = uninstall first; relaunch = same install (cache present)
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
N=${1:-5}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; O=~/bp-gpui-ios/shots/boot.log; : > $O
xcrun simctl boot $U 2>/dev/null
export EXTRAENV="BACKPLANE_PERFORMANCE=1"
one() { # label
  sh ~/bp-gpui-ios/ios-run.sh $U "ob-skip" >/dev/null; sleep 16
  echo "== $1" >> $O
  xcrun simctl spawn $U log show --last 40s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "Backplane boot|BPPerf" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' >> $O
}
i=1; while [ $i -le $N ]; do xcrun simctl uninstall $U dev.backplane.gpui; one fresh$i; i=$((i+1)); done
xcrun simctl uninstall $U dev.backplane.gpui; one prime
i=1; while [ $i -le $N ]; do one relaunch$i; i=$((i+1)); done
xcrun simctl shutdown $U 2>/dev/null
