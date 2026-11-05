#!/bin/sh
# window_e2e.sh : BOAT ONLY. Build the window program, run backplane-hub with bp-policy and it on the pages
# fixture, and compare the windows the hub sends with the JS hub's.
case "$HOME" in /home/h|/home/h/*) echo "window_e2e.sh refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "window_e2e.sh: not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; export BEND_NO_TELEMETRY=1; O=$R/out/we; mkdir -p $O
rm -rf $R/out/we/core && mkdir -p $R/out/we/core/native/host $R/out/we/core/src/core && cp src/core/cbor.bend $R/out/we/core/src/core/ && (cd native/host && tar --exclude=./target -cf - .) | (cd $R/out/we/core/native/host && tar -xf -) && (cd $R/out/we/core/native/host && rm -rf src/agents src/files src/kicad && sed -i 's/mod agents; //; s/mod files; //; s/mod kicad; //; /agents::Agents::start/d; /files::start/d' src/main.rs && cargo build --release --bin backplane-hub > $R/out/we/build-core.log 2>&1); bc=$?; grep -E '^error|Finished' $R/out/we/build-core.log | head -8; [ $bc -eq 0 ] || exit $bc
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -2; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
H=$O/home; rm -rf $H; mkdir -p $H; cp test/gpui/hub/fixtures/pages.jsonl $H/events.jsonl
$R/out/we/core/native/host/target/release/backplane-hub --home $H --port 3815 --policy-bin $R/test/gpui/hub/policy-bin/bp-policy-serve-O2-v3 --window-bin $O/window > $H/log 2>&1 & HP=$!
while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.1; done
bun $R/test/gpui/hub/window_e2e.mjs $H 3815 | tee $O/result.txt
kill $HP
