#!/bin/sh
# The UITextView composer on the simulator against a JS hub on h@backplane (~/bp-gpui-ios-hub, deleted at the end).
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; V=/home/h/bp-gpui-ios-hub
RSYNC_RSH=$M rsync -a $B/bridge/bridge.js hs-mac-mini:bp-gpui-ios/bridge.js
$SB "mkdir -p $V/home $V/proj $V/bin $V/src/web $V/src/core $V/t"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/seed-bots.mjs test/gpui/wire.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
$SB "cd $V && ln -sfn t/tools tools; sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
RSYNC_RSH=$M rsync -a scripts/gpui/ios-composer-mac.sh scripts/gpui/ios-run.sh scripts/gpui/ios-shot.sh hs-mac-mini:bp-gpui-ios/
scripts/gpui/ios-build.sh > $B/results/ios/build-composer.log 2>&1; grep -E "error:|build seconds" $B/results/ios/build-composer.log | head -5
N0=$($SB "grep -c '\"role\":\"user\"' $V/home/events.jsonl")
mkdir -p $B/results/ios/composer $B/shots/ios/composer
$M hs-mac-mini "sh ~/bp-gpui-ios/ios-composer-mac.sh $T"
for f in composer-1.log composer-2.log; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$f $B/results/ios/composer/; done
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/composer-1.png hs-mac-mini:bp-gpui-ios/shots/composer-2.png $B/shots/ios/composer/
$SB "grep '\"role\":\"user\"' $V/home/events.jsonl" | python3 -c "import sys,json;[print(json.dumps(json.loads(l).get('text',''))[:90]) for l in sys.stdin]" > $B/results/ios/composer/user-messages.txt
$SB "grep -c pasted-1.png $V/home/events.jsonl" > $B/results/ios/composer/attach-count.txt
echo "user messages before: $N0" > $B/results/ios/composer/baseline.txt
$SB "sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
cat $B/results/ios/composer/*.log

python3 scripts/gpui/ios-composer-check.py $B/results/ios/composer || { echo "COMPOSER CHECK FAILED"; exit 1; }
