#!/bin/sh
# Reproduce the stuck long-draft send on the real iPhone app against the JS hub (or HUB=rust: the Rust hub with policy-pager/window).
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; V=/home/h/bp-gpui-ios-hub
RSYNC_RSH=$M rsync -a $B/bridge/bridge.js hs-mac-mini:bp-gpui-ios/bridge.js; sha256sum $B/bridge/bridge.js | cut -c1-16
$SB "mkdir -p $V/home $V/proj $V/bin $V/src/web $V/src/core $V/t"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/seed-bots.mjs test/gpui/wire.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
$SB "cd $V && ln -sfn t/tools tools; sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
for n in 60 70; do python3 -c "print(('lorem ipsum dolor sit amet ' * 4000)[:${n}000], end='')" > /tmp/longdraft$n.txt; RSYNC_RSH=$M rsync -a /tmp/longdraft$n.txt hs-mac-mini:bp-gpui-ios/longdraft$n.txt; done
RSYNC_RSH=$M rsync -a scripts/gpui/ios-stuck-mac.sh scripts/gpui/ios-stuck2-mac.sh scripts/gpui/ios-stuck3-mac.sh scripts/gpui/ios-stuck4-mac.sh hs-mac-mini:bp-gpui-ios/
scripts/gpui/ios-build.sh > $B/results/ios/build-stuck.log 2>&1; grep -E "error:|build seconds" $B/results/ios/build-stuck.log | head -2
T0=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 1p | cut -d'\"' -f6")
K=backplane.tail9e0cf2.ts.net:3811
$M hs-mac-mini "SIZES='${SIZES:-0 20 40 60 100}' sh ~/bp-gpui-ios/ios-${MODE:-stuck}-mac.sh $U $T $T0 $K" | tee $B/results/ios/stuck-${MODE:-stuck}-js.txt
mkdir -p $B/shots/ios/stuck; for f in stuck-A1 stuck-A2 stuck-A3 stuck-B1 stuck-B2 stuck-B3 stuck-C1 stuck-C2 stuck-C3 sw60-1 sw60-2 sw70-1 sw70-2 rl60-a rl60-b rl60-c rl70-a rl70-b rl70-c; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$f.png $B/shots/ios/stuck/ 2>/dev/null; done
$SB "python3 - <<'PY'
import json,os
f=os.path.expanduser('$V/home/events.jsonl')
n=[len(json.loads(l).get('text','')) for l in open(f) if l.startswith('{\"\$\":\"MessagePosted\"') and json.loads(l).get('role')=='user']
print('user messages (text lengths):', n)
PY" | tee -a $B/results/ios/stuck-${MODE:-stuck}-js.txt
$SB "sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
