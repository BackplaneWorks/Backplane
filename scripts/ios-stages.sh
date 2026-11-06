#!/bin/sh
# 22 -> 30 -> 300 threads in one project on the throwaway hub (HOME/PROJ/PORT in env), a stage test after each.
cd "$(dirname "$0")/.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; R=$B/ios-hub6; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; O=$B/results/ios
stage() { # tag threadid
  $M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; sh ~/bp-gpui-ios/ios-stage.sh $U $1 3811 ~/bp-gpui-ios/seed6-token $2"
  RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/results/stage-$1.log $O/; RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/stage-$1.png $B/shots/ios/
  curl -s -m5 -H "Authorization: Bearer $(cat $R/home/token)" 100.64.84.41:3811/debug/perf | grep -E "mobile_join|thin.rpc|hub.rpc " > $O/hubperf-$1.txt
}
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl boot $U 2>/dev/null"
stage n22 $(cut -d' ' -f4 $O/bulk-ids-22.txt)
bun scripts/ios-seed.ts 100.64.84.41:3811 $R/home $R/proj bulk 23 30 0 > /dev/null 2>&1
stage n30 $(cut -d' ' -f4 $O/bulk-ids-22.txt)
bun scripts/ios-seed.ts 100.64.84.41:3811 $R/home $R/proj bulk 31 300 0 > /dev/null 2>&1
grep -c '"ThreadCreated"' $R/home/events.jsonl > $O/thread-count-300.txt
stage n300 $(cut -d' ' -f4 $O/bulk-ids-22.txt)
