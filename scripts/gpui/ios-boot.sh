#!/bin/sh
# Start-up from process start on an optimised build: ios-boot.sh [debug|release] [RUNS]. JS hub on h@backplane (~/bp-gpui-ios-hub, deleted at the end).
cd "$(dirname "$0")/../.." || exit 1
CFG=${1:-release}; RUNS=${2:-5}
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; V=/home/h/bp-gpui-ios-hub
RSYNC_RSH=$M rsync -a $B/bridge/bridge.js hs-mac-mini:bp-gpui-ios/bridge.js
$SB "mkdir -p $V/home $V/proj $V/bin $V/src/web $V/src/core $V/t"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/seed-bots.mjs test/gpui/wire.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
$SB "cd $V && ln -sfn t/tools tools; sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
RSYNC_RSH=$M rsync -a scripts/gpui/ios-boot-mac.sh scripts/gpui/ios-run.sh scripts/gpui/ios-shot.sh hs-mac-mini:bp-gpui-ios/
if [ "$CFG" = release ]; then scripts/gpui/ios-build.sh release > $B/results/ios/build-boot.log 2>&1; APPENV='APP=$HOME/bp-gpui-ios/ios/build/ddr/Build/Products/Release-iphonesimulator/Backplane.app'
else scripts/gpui/ios-build.sh > $B/results/ios/build-boot.log 2>&1; APPENV=; fi
grep -E "error:|build seconds" $B/results/ios/build-boot.log | head -5
mkdir -p $B/results/ios/boot
$M hs-mac-mini "$APPENV sh ~/bp-gpui-ios/ios-boot-mac.sh $RUNS"
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/boot.log $B/results/ios/boot/boot-$CFG.log
$SB "sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
python3 scripts/gpui/ios-boot-table.py $B/results/ios/boot/boot-$CFG.log
