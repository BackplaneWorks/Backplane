#!/bin/sh
# On the Mac. ios-stage.sh UDID TAG PORT TOKFILE THREADID: fresh pair, open a thread, send, page the sidebar twice.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=$1; TAG=$2; PORT=$3; export TOKFILE=$4; ID=$5
O=~/bp-gpui-ios/results; mkdir -p $O
export HUBURL=http://${HUBHOST:-backplane.tail9e0cf2.ts.net}:$PORT/ SIMCTL_CHILD_BACKPLANE_PERFORMANCE=1
KEY=${HUBHOST:-backplane.tail9e0cf2.ts.net}:$PORT
xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1
xcrun simctl uninstall $U dev.backplane.gpui >/dev/null 2>&1
sleep 1
sh ~/bp-gpui-ios/ios-run.sh $U "ob-skip;;@pager;;select=$KEY|$ID;;@type=round trip $TAG;;@send;;@back;;@pager;;@pagenext;;@pager;;@pagenext;;@pager;;@pagenext;;@pager" >/dev/null
sleep 50
xcrun simctl io $U screenshot ~/bp-gpui-ios/shots/stage-$TAG.png 2>/dev/null
xcrun simctl spawn $U log show --last 70s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "BPPerf|Backplane pager|view failed|connection retry" | sed 's/^[^ ]* [^ ]* [A-Za-z]* *Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' > $O/stage-$TAG.log
PID=$(xcrun simctl spawn $U launchctl list | grep dev.backplane.gpui | awk '{print $1}')
echo "footprint $(footprint $PID 2>/dev/null | awk '/phys_footprint:/ {print $2,$3}')" >> $O/stage-$TAG.log
