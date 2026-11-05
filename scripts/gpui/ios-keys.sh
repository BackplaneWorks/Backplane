#!/bin/sh
# Composer cost by implementation and draft size (1, 2, 4, 8 KB): key cost, main-thread stalls, screen-update (display stage) cost
# while the hub streams into the open thread. JS hub on h@backplane (~/bp-gpui-ios-hub, deleted at the end). KINDS / SIZES override.
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; V=/home/h/bp-gpui-ios-hub
KINDS=${KINDS:-swiftui isolated uikit}; SIZES=${SIZES:-0 1 2 4 8}
RSYNC_RSH=$M rsync -a $B/bridge/bridge.js hs-mac-mini:bp-gpui-ios/bridge.js
$SB "mkdir -p $V/home $V/proj $V/bin $V/src/web $V/src/core $V/t"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/wire.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
$SB "cd $V && ln -sfn t/tools tools; BP_STANDIN_MODE=stream BP_STREAM_N=40 BP_STREAM_MS=100 sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
RSYNC_RSH=$M rsync -a scripts/gpui/ios-keys-mac.sh scripts/gpui/ios-run.sh scripts/gpui/ios-shot.sh hs-mac-mini:bp-gpui-ios/
scripts/gpui/ios-build.sh > $B/results/ios/build-keys.log 2>&1; grep -E "error:|build seconds" $B/results/ios/build-keys.log | head -2
mkdir -p $B/results/ios/keys
for k in $KINDS; do for n in $SIZES; do
  $M hs-mac-mini "sh ~/bp-gpui-ios/ios-keys-mac.sh $k $n $T" &
  PID=$!
  sleep 20
  $SB "\$HOME/.bun/bin/bun $V/tools/ios-seed.ts 100.96.192.111:3811 $V/home $V/proj $T 'stream $k $n'" > /dev/null 2>&1
  wait $PID
  RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/keys-$k-$n.log $B/results/ios/keys/
done; done
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/keys-*-*.png $B/shots/ios/keys/ 2>/dev/null
$SB "sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
python3 - <<'PY'
import re,glob,statistics as st,os
B='build/gpui/results/ios/keys'
print("kind     KB  key cost ms (median/max)  stalls>30ms (n/max ms)  screen-update display ms (n/median/p95/max)")
for f in sorted(glob.glob(B+'/keys-*.log')):
    k,n=os.path.basename(f)[5:-4].rsplit('-',1)
    keys=[float(x) for x in re.findall(r'key cost ([\d.]+) ms',open(f).read())]
    stalls=[int(x) for x in re.findall(r'stall (\d+) ms',open(f).read())]
    disp=[]
    for l in open(f):
        m=re.match(r'BPPerf (\d+) (\d+) (\d+) (\d+) (\d+) (\d+)',l)
        if m and int(m.group(1))==8: disp.append((int(m.group(4))-int(m.group(3)))/1e6)
    disp.sort()
    p=lambda q: round(disp[min(len(disp)-1,int(len(disp)*q))]) if disp else None
    print(f"{k:9s}{n:>3s}  {round(st.median(keys),1) if keys else '-'}/{max(keys) if keys else '-'}   {len(stalls)}/{max(stalls) if stalls else 0}   {len(disp)}/{p(.5)}/{p(.95)}/{round(disp[-1]) if disp else None}")
PY
