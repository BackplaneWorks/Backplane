#!/bin/sh
# The mention/reference popup (@ > % # $) on the real iPhone app with the bridge from build/gpui/bridge/bridge.js and the JS hub
# build/gpui/hub/android/backplane-serve, a hub of our own on h@backplane (~/bp-gpui-ios-hub, deleted at the end).
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; V=/home/h/bp-gpui-ios-hub
RSYNC_RSH=$M rsync -a $B/bridge/bridge.js hs-mac-mini:bp-gpui-ios/bridge.js
$SB "mkdir -p $V/home $V/proj $V/bin $V/tools $V/src/web $V/src/core $V/t"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/seed-bots.mjs test/gpui/wire.mjs test/gpui/set-setting.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
$SB "cd $V && ln -sfn t/tools tools; sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1; (cd t/tools && \$B seed-bots.mjs $V/home \$HP); sleep 2; echo bots \$(grep -c '\"BotSet\"' home/events.jsonl)"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
scripts/gpui/ios-build.sh > $B/results/ios/build-popup.log 2>&1; grep -E "error:|build seconds" $B/results/ios/build-popup.log | head -2
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token; xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
sh ~/bp-gpui-ios/ios-shot.sh $U popup-at 40 'ob-skip;;@type=@B' $T
sh ~/bp-gpui-ios/ios-shot.sh $U popup-gt 40 'ob-skip;;@type=>l' $T
sh ~/bp-gpui-ios/ios-shot.sh $U popup-slash 25 'ob-skip;;@type=/' $T
xcrun simctl shutdown $U"
for n in popup-at popup-gt popup-slash; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$n.png $B/shots/ios/; done
$SB "echo events: project \$(grep -c ProjectCreated $V/home/events.jsonl) threads \$(grep -c ThreadCreated $V/home/events.jsonl) bots \$(grep -c '\"BotSet\"' $V/home/events.jsonl); sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
