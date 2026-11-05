#!/bin/sh
# On the Mac: the UITextView composer's behaviours, driven by debug acts. usage: ios-composer-mac.sh THREAD
# Run 1: IME composition (marked text), external change during composition, commit, undo, text insert (paste), selection replace,
# image paste, hardware Return (send), undo after send, then a draft left in place. Run 2 (relaunch, no reinstall): the draft is back.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
T=$1; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}
xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
export EXTRAENV="BACKPLANE_PERFORMANCE=1"
A="ob-skip;;@type=hello world;;@cstate=a1;;@ime=ni;;@type=EXTERNAL;;@cstate=a2;;@ime=ab;;@cstate=a3;;@send;;@cstate=a4;;@type=hello world;;@ime=ni;;@imecommit=NIHAO;;@cstate=b1;;@undo;;@cstate=b2;;@insert= pasted;;@cstate=b3;;@sel=0,3;;@insert=XY;;@cstate=b4;;@pasteimg;;@cstate=b5;;@hwreturn;;@cstate=b6;;@undo;;@cstate=b7;;@type=draft kept across relaunch;;@cstate=c1"
sh ~/bp-gpui-ios/ios-shot.sh $U composer-1 80 "$A" $T > /dev/null
xcrun simctl spawn $U log show --last 120s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "Backplane composer|Backplane attach|crash|Fatal" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' > ~/bp-gpui-ios/shots/composer-1.log
sh ~/bp-gpui-ios/ios-shot.sh $U composer-2 15 "ob-skip;;@cstate=r1" $T > /dev/null
xcrun simctl spawn $U log show --last 40s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "Backplane composer" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' > ~/bp-gpui-ios/shots/composer-2.log
xcrun simctl shutdown $U 2>/dev/null
