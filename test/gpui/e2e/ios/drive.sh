#!/bin/sh
# Desktop-side launcher (ssh and rsync only, nothing heavy here): fixture hub on h@backplane, the journeys on the Mac, results back.
#   drive.sh JOURNEY|all [HUB_BIN]     HUB_BIN: a hub binary in this repo (default build/gpui/hub/android/backplane-serve, the JS hub)
#   env POLICY: the Rust hub's --policy-bin (default build/gpui/slim/bp-policy-pager; slim's popup/room/Files policy: build/gpui/slim/bp-policy-bp)
# Start it with work_run {lock: "mac-heavy"}. Results: build/gpui/results/ios/e2e/ (json, mp4, png, summary.json).
cd "$(dirname "$0")/../../../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}
J=${1:-all}; HUB=${2:-build/gpui/hub/android/backplane-serve}
V=/home/h/bp-gpui-e2e-fixture; HOSTIP=100.96.192.111; PORT=3850; HOSTDNS=backplane.tail9e0cf2.ts.net
OUT=$B/results/ios/e2e; mkdir -p $OUT
$SB "mkdir -p $V/repo/test/tools $V/repo/src/web $V/repo/src/core $V/root"
rsync -a --exclude node_modules --exclude out test/gpui/ h@backplane:bp-gpui-e2e-fixture/repo/test/gpui/
rsync -a test/tools/claude_standin.ts test/tools/perf_standin.ts h@backplane:bp-gpui-e2e-fixture/repo/test/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-e2e-fixture/repo/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-e2e-fixture/repo/src/core/
rsync -a $HUB h@backplane:bp-gpui-e2e-fixture/hub
RUSTFLAGS_=""
case "$HUB" in *backplane-hub-full*)
  # the Rust hub: its policy and window helpers (policy-pager: the bounded sidebar pager) next to it
  POLICY=${POLICY:-build/gpui/slim/bp-policy-pager}
  rsync -a $POLICY build/gpui/hub/new/window h@backplane:bp-gpui-e2e-fixture/
  RUSTFLAGS_="--policy-bin $V/$(basename $POLICY) --window-bin $V/window";; esac
# the hub is started by an ssh session that stays open as long as the fixture lives: run it in the background of THIS script
# (its pid is killed at the end) and never wait for it
ssh -n -o BatchMode=yes h@backplane "cd $V/repo/test/gpui/e2e && PERF_REPO=$V/repo exec systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 \$HOME/.bun/bin/bun serve-fixture.mjs --hub-bin $V/hub --root $V/root --host $HOSTIP --port $PORT $RUSTFLAGS_" < /dev/null > $B/results/ios/e2e-fixture.log 2>&1 &
FIXSSH=$!
sleep 30
tail -2 $B/results/ios/e2e-fixture.log | cut -c1-200
$SB "cat $V/root/home/token" > /tmp/e2e-token
# what the hub replies to files.dir / boards.list (the journeys that wait on them), before the journeys run
$SB "cd $V/repo/test/gpui/e2e && timeout 40 \$HOME/.bun/bin/bun ios/probe.mjs $V/root $((PORT+1000))" > $OUT/probe.txt 2>&1
# a pairing invitation through the hub's tailnet address, as a phone gets it: its routes must be reachable (not loopback); the link is
# one-time and goes to the Mac in a file (never printed); only the routes are kept in the results
$SB "printf 'Authorization: Bearer %s\n' \"\$(cat $V/root/home/token)\" | curl -s -m10 -X POST -H @- -H 'Content-Type: application/json' -d '{}' http://$HOSTIP:$PORT/auth/invitations" > /tmp/e2e-invite.json
python3 -c "import json;j=json.load(open('/tmp/e2e-invite.json'));open('/tmp/e2e-invite','w').write(j.get('link',''));print('invitation routes:',j.get('routes'),'localOnly:',j.get('localOnly'),'error:',j.get('error'))" | tee $OUT/invite-routes.txt
RSYNC_RSH=$M rsync -a /tmp/e2e-invite hs-mac-mini:bp-gpui-ios/e2e-invite
RSYNC_RSH=$M rsync -a /tmp/e2e-token hs-mac-mini:bp-gpui-ios/e2e-token
RSYNC_RSH=$M rsync -a --delete test/gpui/e2e/ios/ hs-mac-mini:bp-gpui-ios/e2e/
$M hs-mac-mini "cd ~/bp-gpui-ios/e2e && rm -rf ~/bp-gpui-ios/e2e-out && SIM_UDID=$U OUT=\$HOME/bp-gpui-ios/e2e-out HUB_HOST=$HOSTDNS HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/bp-gpui-ios/e2e-token HUB_INVITE_FILE=\$HOME/bp-gpui-ios/e2e-invite HUB_CTL=http://$HOSTDNS:$((PORT+1)) LONG_SECS=${LONG_SECS:-300} sh ./run.sh $J; xcrun simctl shutdown $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/e2e-out/ $OUT/
kill $FIXSSH 2>/dev/null
$SB "tail -c 20000 $V/root/hub.log 2>/dev/null; ls $V/root" > $OUT/hub-log-tail.txt 2>&1
$SB "P=\$(ss -ltnp | grep ':$PORT ' | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2); echo fixture pid \$P; [ -n \"\$P\" ] && kill \$P; sleep 5; ss -ltn | grep -cE ':385[01] |:4850 ' ; rm -rf $V; ls -d $V 2>&1 | tail -1"
