#!/bin/sh
# On the Mac: does opening a thread that holds a long draft fail ("Invalid native materialization")? Draft sizes 60,000 (under the
# bridge's 65,536 bound) and 70,000 (over), flow: type, leave the thread, open another, come back.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
U=$1; T=$2; T0=$3; K=$4
xcrun simctl boot $U 2>/dev/null
run() { # name textfile
  L=$(cat ~/bp-gpui-ios/$2)
  xcrun simctl uninstall $U dev.backplane.gpui
  sh ~/bp-gpui-ios/ios-shot.sh $U $1 55 "ob-skip;;@type=$L;;@times;;@back;;@times;;select=$K|$T0;;@times;;@back;;@times;;select=$K|$T" $T > /dev/null
  xcrun simctl spawn $U log show --last 100s --predicate 'process == "Backplane" AND eventMessage CONTAINS "Backplane"' --style compact 2>/dev/null | grep -E "send refused|view failed|connection retry|act dropped" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' | cut -c1-200 > ~/bp-gpui-ios/shots/$1.log
  echo "== $1 ($2): $(grep -c 'view failed' ~/bp-gpui-ios/shots/$1.log) view failures"; head -3 ~/bp-gpui-ios/shots/$1.log
}
for i in 1 2; do run sw60-$i longdraft60.txt; run sw70-$i longdraft70.txt; done
xcrun simctl shutdown $U
