#!/bin/sh
# On the Mac: relaunch with a long draft persisted (typed in the previous launch): does the thread ever finish loading, does a send go?
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
U=$1; T=$2
xcrun simctl boot $U 2>/dev/null
run() { # name textfile
  L=$(cat ~/bp-gpui-ios/$2)
  xcrun simctl uninstall $U dev.backplane.gpui
  # launch 1: type the draft (it is kept in the app's storage), then kill the app
  sh ~/bp-gpui-ios/ios-shot.sh $U $1-a 25 "ob-skip;;@type=$L" $T > /dev/null
  xcrun simctl terminate $U dev.backplane.gpui; sleep 2
  # launch 2: the draft comes back from storage; the thread opens; 60 s later: a picture, then send
  sh ~/bp-gpui-ios/ios-shot.sh $U $1-b 60 "ob-skip" $T > /dev/null
  xcrun simctl spawn $U log show --last 100s --predicate 'process == "Backplane" AND eventMessage CONTAINS "Backplane"' --style compact 2>/dev/null | grep -E "send refused|view failed|connection retry|act dropped" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' | cut -c1-200 > ~/bp-gpui-ios/shots/$1.log
  xcrun simctl terminate $U dev.backplane.gpui; sleep 2
  sh ~/bp-gpui-ios/ios-shot.sh $U $1-c 45 "ob-skip;;@times;;@times;;@send" $T > /dev/null
  xcrun simctl spawn $U log show --last 80s --predicate 'process == "Backplane" AND eventMessage CONTAINS "Backplane"' --style compact 2>/dev/null | grep -E "send refused|view failed|connection retry" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' | cut -c1-200 > ~/bp-gpui-ios/shots/$1-send.log
  echo "== $1 ($2): launch-2 view failures $(grep -c 'view failed' ~/bp-gpui-ios/shots/$1.log), send log: $(cat ~/bp-gpui-ios/shots/$1-send.log | head -2 | tr '\n' '|')"
}
run rl60 longdraft60.txt
run rl70 longdraft70.txt
xcrun simctl shutdown $U
