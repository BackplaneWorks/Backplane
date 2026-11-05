#!/bin/sh
# A draft over 64 KB (the bridge's view.update bound, 0d930e939) on the real iPhone app, plus the @/> popups, with the bridge in
# build/gpui/bridge/bridge.js and the JS hub build/gpui/hub/android/backplane-serve on h@backplane (~/bp-gpui-ios-hub, deleted at the end).
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; V=/home/h/bp-gpui-ios-hub
sha256sum $B/bridge/bridge.js | tee $B/results/ios/longdraft-bridge.sha
RSYNC_RSH=$M rsync -a $B/bridge/bridge.js hs-mac-mini:bp-gpui-ios/bridge.js
$SB "mkdir -p $V/home $V/proj $V/bin $V/src/web $V/src/core $V/t"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/seed-bots.mjs test/gpui/wire.mjs test/gpui/set-setting.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
$SB "cd $V && ln -sfn t/tools tools; sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1; (cd t/tools && \$B seed-bots.mjs $V/home \$HP); sleep 2; echo bots \$(grep -c '\"BotSet\"' home/events.jsonl)"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
T0=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 1p | cut -d'\"' -f6")
K=backplane.tail9e0cf2.ts.net:3811
for n in 30 70 100; do python3 -c "print(('lorem ipsum dolor sit amet ' * 4000)[:${n}000], end='')" > /tmp/longdraft$n.txt; RSYNC_RSH=$M rsync -a /tmp/longdraft$n.txt hs-mac-mini:bp-gpui-ios/longdraft$n.txt; done
RSYNC_RSH=$M rsync -a scripts/gpui/ios-longdraft-mac.sh hs-mac-mini:bp-gpui-ios/
scripts/gpui/ios-build.sh > $B/results/ios/build-longdraft.log 2>&1; grep -E "error:|build seconds" $B/results/ios/build-longdraft.log | head -2
$M hs-mac-mini "sh ~/bp-gpui-ios/ios-longdraft-mac.sh $U $T"
for n in ld-sent-30 ld-sent-70 ld-sent-100; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$n.png $B/shots/ios/; done
$SB "python3 - <<'PY'
import json,os
f=os.path.expanduser('$V/home/events.jsonl')
n=[len(json.loads(l).get('text','')) for l in open(f) if l.startswith('{\"\$\":\"MessagePosted\"') and json.loads(l).get('role')=='user']
print('user messages (text lengths):', n)
PY" | tee $B/results/ios/longdraft-hub.txt
$SB "sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
