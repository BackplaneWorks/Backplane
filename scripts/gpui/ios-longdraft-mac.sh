#!/bin/sh
# On the Mac: send a long draft (30,000 / 70,000 / 100,000 characters, read from files: too long for an ssh argument) from the iPhone app.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
U=$1; T=$2
xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
for n in 30 70 100; do
  LONG=$(cat ~/bp-gpui-ios/longdraft$n.txt)
  sh ~/bp-gpui-ios/ios-shot.sh $U ld-sent-$n 75 "ob-skip;;@type=$LONG;;@times;;@times;;@times;;@times;;@send" $T
done
xcrun simctl shutdown $U
