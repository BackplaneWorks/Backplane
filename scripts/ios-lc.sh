#!/bin/sh
# On the Mac. ios-lc.sh UDID DEV SCHEME: iOS shots of the logo and cats with their frames (BACKPLANE_GEOM) for the visual check.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=$1; DEV=$2; S=$3
O=~/bp-gpui-ios/lc; mkdir -p $O
export HUBURL=http://${HUBHOST:-backplane.tail9e0cf2.ts.net}:${HUBPORT:-3811}/ TOKFILE=~/bp-gpui-ios/seed7-token GEOM=1
xcrun simctl boot $U 2>/dev/null; xcrun simctl bootstatus $U -b >/dev/null 2>&1
shot() { # page acts nolink
  p=$1; acts=$2; sel=$4
  xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1; xcrun simctl uninstall $U dev.backplane.gpui >/dev/null 2>&1
  if [ "$3" = nolink ]; then LAUNCHARGS="-app.appearance $S" NOLINK=1 sh ~/bp-gpui-ios/ios-run.sh $U "$acts" >/dev/null; else sh ~/bp-gpui-ios/ios-run.sh $U "$acts" $sel >/dev/null; fi
  sleep 16
  xcrun simctl io $U screenshot $O/ios-$DEV-$S-$p.png 2>/dev/null
  xcrun simctl spawn $U log show --last 20s --predicate 'process == "Backplane" AND eventMessage CONTAINS "Backplane geom"' --style compact 2>/dev/null | sed -n 's/.*Backplane geom //p' > $O/ios-$DEV-$S-$p.geom
}
shot list 'ob-skip'
shot bots 'ob-skip;;@tab=bots'
shot bot 'ob-skip;;@tab=bots;;@open=Bub'
shot look 'ob-skip;;@tab=bots;;@open=Bub;;bot-tab=settings'
shot room 'ob-skip;;@tab=bots;;@open=crew'
THREAD=$(cat ~/bp-gpui-ios/lc-thread.txt 2>/dev/null)
[ -n "$THREAD" ] && shot popup 'ob-skip;;@type=@' "" "$THREAD"
shot pair '' nolink
xcrun simctl shutdown $U
