#!/bin/sh
# 22 then 300 threads, the real iPhone app against the Rust hub (backplane-hub-full) on h@backplane:3815, synthetic events.
# Everything is created under ~/bp-gpui-ios-hub on h@backplane and deleted at the end. Shots/logs: build/gpui/{shots,results}/ios/rust2-*
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F
V=/home/h/bp-gpui-ios-hub
$SB "mkdir -p $V/rust/home $V/rust/proj $V/bin $V/tools $V/src/web $V/src/core"
rsync -a $B/hub/new/backplane-hub-full $B/hub/new/window $B/slim/bp-policy-page h@backplane:bp-gpui-ios-hub/rust/
rsync -a test/tools/claude_standin.ts test/gpui/wire.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a /tmp/seedrust.py h@backplane:bp-gpui-ios-hub/seedrust.py
start="cd $V/rust/proj; export BACKPLANE_AGENTS=$V/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain TZ=Etc/UTC; nohup systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 ../backplane-hub-full --home ../home --port 3815 --host 100.96.192.111 --no-tailscale --foreground --policy-bin ../bp-policy-page --window-bin ../window > ../hub.log 2>&1 & sleep 8; P=\$(ss -ltnp | grep ':3815 ' | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2); echo \$P > ../pid; tr '\\0' '\\n' < /proc/\$P/environ | grep BACKPLANE_AGENTS"
stop="kill \$(cat $V/rust/pid); sleep 3"
$SB "printf '#!/bin/sh\nexec \$HOME/.bun/bin/bun $V/tools/claude_standin.ts \"\$@\"\n' > $V/bin/claude; for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $V/bin/\$n; done; chmod +x $V/bin/*; git -C $V/rust/proj init -q; $start"
$SB "cd $V; \$HOME/.bun/bin/bun tools/ios-seed.ts 100.96.192.111:3815 $V/rust/home $V/rust/proj >/dev/null 2>&1; grep -c ProjectCreated rust/home/events.jsonl"
$SB "$stop; python3 $V/seedrust.py 22; $start" | tail -1
$SB "cat $V/rust/home/token" > /tmp/rust-token; RSYNC_RSH=$M rsync -a /tmp/rust-token hs-mac-mini:bp-gpui-ios/seed-rust-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/rust/home/events.jsonl | sed -n 3p | cut -d'\"' -f6")
for N in 22 300; do
  if [ $N = 300 ]; then $SB "$stop; python3 $V/seedrust.py 300 23; $start" | tail -1; fi
  $M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl boot $U 2>/dev/null; sh ~/bp-gpui-ios/ios-stage.sh $U rust2-$N 3815 ~/bp-gpui-ios/seed-rust-token $T; xcrun simctl shutdown $U"
  RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/results/stage-rust2-$N.log $B/results/ios/; RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/stage-rust2-$N.png $B/shots/ios/
done
$SB "$stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
