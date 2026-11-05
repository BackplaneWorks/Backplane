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
print("kind          KB | key cost ms median/p90/max | hop-to-main ms median | sizeThatFits ms median | stalls>30ms in typing (n/max)")
def med(v): return round(st.median(v),1) if v else '-'
for f in sorted(glob.glob(B+'/keys-*.log')):
    k,n=os.path.basename(f)[5:-4].rsplit('-',1) if not os.path.basename(f)[5:-4].endswith('-slow') else (os.path.basename(f)[5:-4].rsplit('-',1))
    base=os.path.basename(f)[5:-4]; n=base.rsplit('-',1)[1]; k=base.rsplit('-',1)[0]
    if not n.isdigit(): continue
    n=int(n); lo,hi=n*1000,n*1000+10; T=open(f).read()
    ok=lambda d: lo<=int(d)<=hi and (n>0 or int(d)<=10)
    keys=sorted(float(c) for c,d in re.findall(r'key cost ([\d.]+) ms draft=(\d+)',T) if ok(d))
    hop=[float(h) for h,d in re.findall(r'hop to main ([\d.]+) ms, apply [\d.]+ ms draft=(\d+)',T) if ok(d)]
    sz=[float(c) for c,d in re.findall(r'sizeThatFits ([\d.]+) ms bytes=(\d+)',T) if ok(d)]
    stalls=[int(s) for s,d in re.findall(r'stall (\d+) ms draft=(\d+)',T) if ok(d)]
    p9=keys[min(len(keys)-1,int(len(keys)*.9))] if keys else '-'
    print(f"{k:12s}{n:>3} | {med(keys)}/{p9}/{keys[-1] if keys else '-'} | {med(hop)} | {med(sz)} | {len(stalls)}/{max(stalls) if stalls else 0}")
PY
