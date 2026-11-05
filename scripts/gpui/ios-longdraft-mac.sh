#!/bin/sh
# On the Mac: the over-64 KB draft shots (the 70,000-character text is read from ~/bp-gpui-ios/longdraft.txt: too long for an ssh argument).
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
U=$1; T=$2; T0=$3; K=$4
LONG=$(cat ~/bp-gpui-ios/longdraft.txt)
xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
sh ~/bp-gpui-ios/ios-shot.sh $U ld-typed 40 "ob-skip;;@type=$LONG" $T
sh ~/bp-gpui-ios/ios-shot.sh $U ld-switched 50 "ob-skip;;@type=$LONG;;@back;;select=$K|$T0;;@back;;select=$K|$T" $T
xcrun simctl terminate $U dev.backplane.gpui; sleep 2
sh ~/bp-gpui-ios/ios-shot.sh $U ld-restart 30 'ob-skip' $T
sh ~/bp-gpui-ios/ios-shot.sh $U ld-sent 60 "ob-skip;;@type=$LONG;;@send" $T
sh ~/bp-gpui-ios/ios-shot.sh $U pop-at 35 'ob-skip;;@type=@B' $T
sh ~/bp-gpui-ios/ios-shot.sh $U pop-gt 35 'ob-skip;;@type=>l' $T
xcrun simctl shutdown $U
