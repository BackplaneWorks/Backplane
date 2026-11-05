#!/bin/sh
# Desktop-side launcher (ssh and rsync only, nothing heavy here): fixture hub on h@backplane, the journeys on the Mac, results back.
#   drive.sh JOURNEY|all [HUB_BIN]     HUB_BIN: a hub binary in this repo (default build/gpui/hub/android/backplane-serve, the JS hub)
# Start it with work_run {lock: "mac-heavy"}. Results: build/gpui/results/ios/e2e/ (json, mp4, png, summary.json).
cd "$(dirname "$0")/../../../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}
J=${1:-all}; HUB=${2:-build/gpui/hub/android/backplane-serve}
V=/home/h/bp-gpui-e2e-fixture; HOSTIP=100.96.192.111; PORT=3850; HOSTDNS=backplane.tail9e0cf2.ts.net
OUT=$B/results/ios/e2e; mkdir -p $OUT
$SB "mkdir -p $V/repo/test/tools $V/repo/src/web $V/repo/src/core $V/root"
rsync -a --exclude node_modules --exclude out test/gpui/ h@backplane:bp-gpui-e2e-fixture/repo/test/gpui/
rsync -a test/tools/claude_standin.ts h@backplane:bp-gpui-e2e-fixture/repo/test/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-e2e-fixture/repo/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-e2e-fixture/repo/src/core/
rsync -a $HUB h@backplane:bp-gpui-e2e-fixture/hub
$SB "cd $V/repo/test/gpui/e2e && nohup systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 \$HOME/.bun/bin/bun serve-fixture.mjs --hub-bin $V/hub --root $V/root --host $HOSTIP --port $PORT > $V/fixture.log 2>&1 &
sleep 25; tail -3 $V/fixture.log | cut -c1-200"
$SB "cat $V/root/home/token" > /tmp/e2e-token
RSYNC_RSH=$M rsync -a /tmp/e2e-token hs-mac-mini:bp-gpui-ios/e2e-token
RSYNC_RSH=$M rsync -a --delete test/gpui/e2e/ios/ hs-mac-mini:bp-gpui-ios/e2e/
$M hs-mac-mini "cd ~/bp-gpui-ios/e2e && rm -rf ~/bp-gpui-ios/e2e-out && SIM_UDID=$U OUT=\$HOME/bp-gpui-ios/e2e-out HUB_HOST=$HOSTDNS HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/bp-gpui-ios/e2e-token HUB_CTL=http://$HOSTDNS:$((PORT+1)) LONG_SECS=\${LONG_SECS:-300} sh ./run.sh $J; xcrun simctl shutdown $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/e2e-out/ $OUT/
$SB "P=\$(ss -ltnp | grep ':$PORT ' | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2); echo fixture pid \$P; kill \$P; sleep 5; ss -ltn | grep -cE ':385[01] |:4850 ' ; rm -rf $V; ls -d $V 2>&1 | tail -1"
