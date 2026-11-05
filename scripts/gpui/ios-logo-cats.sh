#!/bin/sh
# iOS logo + cats evidence, one scheme: ios-logo-cats.sh dark|light (hub on 3811 with the bots seeded, see docs below)
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; R=$B/ios-hub8; S=$1
bun test/gpui/set-setting.mjs $R/home 100.64.84.41:3811 ui.theme $S; sleep 2
RSYNC_RSH=$M rsync -a scripts/gpui/ios-lc.sh hs-mac-mini:bp-gpui-ios/
grep -o '"ThreadCreated","id":"[^"]*"' $R/home/events.jsonl | head -1 | cut -d'"' -f6 > /tmp/lc-thread.txt; RSYNC_RSH=$M rsync -a /tmp/lc-thread.txt hs-mac-mini:bp-gpui-ios/lc-thread.txt
$M hs-mac-mini "sh ~/bp-gpui-ios/ios-lc.sh E3BC1B9E-AE4D-498C-97AF-B22FE897B41F phone $S; sh ~/bp-gpui-ios/ios-lc.sh 75A16A8E-C40D-40C2-ABFA-F5E403486F10 ipad $S"
mkdir -p $B/results/ios/lc; RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/lc/ $B/results/ios/lc/
