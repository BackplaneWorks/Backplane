#!/bin/sh
# window_e2e.sh : BOAT ONLY. Build the window program, run backplane-hub with bp-policy and it on the pages
# fixture, and compare the windows the hub sends with the JS hub's.
case "$HOME" in /home/h|/home/h/*) echo "window_e2e.sh refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "window_e2e.sh: not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; export BEND_NO_TELEMETRY=1; O=$R/out/we; mkdir -p $O
T=$HOME/bins-target; (cd native/host && rm -f $T/release/backplane-hub && CARGO_TARGET_DIR=$T cargo build --release --bin backplane-hub > $R/out/we/build-hub.log 2>&1); bc=$?; grep -E '^error|Finished' $R/out/we/build-hub.log | head -8; [ $bc -eq 0 ] && { cp $T/release/backplane-hub $O/backplane-hub || bc=1; }; [ $bc -eq 0 ] || exit $bc
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -2; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
H=$O/home; rm -rf $H; mkdir -p $H; cp test/gpui/hub/fixtures/pages.jsonl $H/events.jsonl
$O/backplane-hub --home $H --port 3815 --policy-bin $R/test/gpui/hub/policy-bin/bp-policy-serve-O2-v3 --window-bin $O/window > $H/log 2>&1 & HP=$!
n=0; while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.1; n=$((n+1)); [ $n -gt 300 ] && { echo 'hub did not start'; tail -5 $H/log; kill $HP; exit 1; }; done
bun $R/test/gpui/hub/window_e2e.mjs $H 3815 > $O/result.txt 2>&1; e=$?; cat $O/result.txt; kill $HP; exit $e
