#!/bin/sh
# On the Mac: the stuck long-draft send. Variants of the flow, each 3 times, then which sends reached the hub is read there.
#   A multi-type: type the 70,000 characters twice, 2 s between, send after 2 s     (the original flow)
#   B one type, send after 2 s    C one type, send after 8 s
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
U=$1; T=$2; L=$(cat ~/bp-gpui-ios/longdraft70.txt)
xcrun simctl boot $U 2>/dev/null
run() { # name acts
  xcrun simctl uninstall $U dev.backplane.gpui
  sh ~/bp-gpui-ios/ios-shot.sh $U $1 45 "$2" $T > /dev/null
  xcrun simctl spawn $U log show --last 1m --predicate 'process == "Backplane" AND eventMessage CONTAINS "Backplane"' --style compact 2>/dev/null | grep -E "send refused|view failed|connection retry|act dropped" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' | cut -c1-260 > ~/bp-gpui-ios/shots/$1.log
  echo "== $1"; cat ~/bp-gpui-ios/shots/$1.log | head -4
}
for i in 1 2 3; do
  run stuck-A$i "ob-skip;;@type=$L;;@times;;@type=$L;;@times;;@send"
  run stuck-B$i "ob-skip;;@type=$L;;@times;;@send"
  run stuck-C$i "ob-skip;;@type=$L;;@times;;@times;;@times;;@times;;@send"
done
xcrun simctl shutdown $U
