#!/bin/sh
# The real iPhone app against the Rust hub on h@backplane (~/bp-gpui-ios-hub/rust, port 3815, synthetic events): open a
# thread, reconnect with a draft, then 300 threads and the sidebar pager. Shots in build/gpui/shots/ios/rust-*.png
cd "$(dirname "$0")/.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; SB='ssh -o BatchMode=yes h@backplane'
T=$(cat /tmp/rust-thread)
start='cd ~/bp-gpui-ios-hub/rust/proj; export BACKPLANE_AGENTS=$HOME/bp-gpui-ios-hub/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain TZ=Etc/UTC; nohup systemd-run --user --scope -q ../backplane-hub-full --home ../home --port 3815 --host 100.96.192.111 --no-tailscale --foreground --policy-bin ../bp-policy-page --window-bin ../window > ../hub.log 2>&1 & sleep 8; P=$(ss -ltnp | grep ":3815 " | grep -o "pid=[0-9]*" | head -1 | cut -d= -f2); echo pid $P > ../pid; tr "\0" "\n" < /proc/$P/environ | grep BACKPLANE_AGENTS'
stop='cd ~/bp-gpui-ios-hub/rust; kill $(grep -o "[0-9]*" pid | head -1); sleep 3'
E1="export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3815/ TOKFILE=~/bp-gpui-ios/seed-rust-token"
$M hs-mac-mini "$E1; xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui; sh ~/bp-gpui-ios/ios-shot.sh $U rust-open 25 'ob-skip' $T"
# reconnect: a draft, the hub killed, then back
$M hs-mac-mini "$E1; sh ~/bp-gpui-ios/ios-shot.sh $U rust-reconnect-1 18 '@type=draft kept over a reconnect' $T"
$SB "$stop"
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; sleep 12; xcrun simctl io $U screenshot ~/bp-gpui-ios/shots/rust-reconnect-2.png 2>/dev/null"
$SB "$start" | tail -1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; sleep 35; xcrun simctl io $U screenshot ~/bp-gpui-ios/shots/rust-reconnect-3.png 2>/dev/null"
# 300 threads and the pager
$SB "$stop"; $SB 'python3 ~/bp-gpui-ios-hub/seedrust.py 300 23'; $SB "$start" | tail -1
$M hs-mac-mini "$E1; sh ~/bp-gpui-ios/ios-stage.sh $U rust300 3815 ~/bp-gpui-ios/seed-rust-token $T; xcrun simctl shutdown $U"
for n in rust-open rust-reconnect-1 rust-reconnect-2 rust-reconnect-3 stage-rust300; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$n.png $B/shots/ios/; done
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/results/stage-rust300.log $B/results/ios/
