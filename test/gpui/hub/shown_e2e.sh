#!/bin/sh
# shown_e2e.sh : BOAT ONLY. Build the window program, run backplane-hub with bp-policy and it on the pages
# fixture, and compare the windows the hub sends with the JS hub's.
case "$HOME" in /home/h|/home/h/*) echo "shown_e2e.sh refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "shown_e2e.sh: not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; export BEND_NO_TELEMETRY=1; O=$R/out/sh; mkdir -p $O
(cd native/host && rm -rf src/agents src/files src/kicad && sed -i 's/mod agents; //; s/mod files; //; s/mod kicad; //; /agents::Agents::start/d; /files::start/d' src/main.rs && cargo build --release --bin backplane-hub 2>&1 | grep -E '^error|Finished' | head -5)
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -2; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
H=$O/home; rm -rf $H; mkdir -p $H; cp test/gpui/hub/fixtures/pages.jsonl $H/events.jsonl
$R/native/host/target/release/backplane-hub --home $H --port 3815 --policy-bin $R/test/gpui/hub/policy-bin/bp-policy-page --window-bin $O/window > $H/log 2>&1 & HP=$!
n=0; while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.1; n=$((n+1)); [ $n -gt 300 ] && { echo 'hub did not start'; tail -5 $H/log; kill $HP; exit 1; }; done
VERBOSE=1 bun $R/test/gpui/hub/shown_e2e.mjs $H 3815 | tee $O/result.txt
kill $HP
