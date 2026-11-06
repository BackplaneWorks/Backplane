#!/bin/sh
# sidebar_page_e2e.sh : BOAT ONLY. Build the window program, run backplane-hub with bp-policy and it on the pages
# fixture, and compare the windows the hub sends with the JS hub's.
case "$HOME" in /home/h|/home/h/*) echo "sidebar_page_e2e.sh refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "sidebar_page_e2e.sh: not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; export BEND_NO_TELEMETRY=1; O=$R/out/sp; mkdir -p $O
mkdir -p out; (cd native/host && rm -f target/release/backplane-hub && cargo build --release --bin backplane-hub > ../../out/hubbuild.log 2>&1); bh=$?; grep -E '^error|Finished' out/hubbuild.log | head -5; [ $bh -eq 0 ] || exit $bh
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -2; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
H=$O/home; rm -rf $H; mkdir -p $H
python3 - > $H/events.jsonl <<'PY'
import json
print(json.dumps({"$":"ProjectCreated","id":"p1","title":"bulk","root":"/x","at":1}))
for i in range(300):
    print(json.dumps({"$":"ThreadCreated","id":f"t{i}","project":"p1","title":f"bulk {i+1}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2+i}))
    print(json.dumps({"$":"MessagePosted","thread":f"t{i}","msg":f"m{i}","role":"user","text":f"hello {i}","at":2+i}))
PY
$R/native/host/target/release/backplane-hub --home $H --port 3815 --policy-bin $R/test/gpui/hub/policy-bin/bp-policy-page --window-bin $O/window > $H/log 2>&1 & HP=$!
n=0; while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.1; n=$((n+1)); [ $n -gt 300 ] && { echo 'hub did not start'; tail -5 $H/log; kill $HP; exit 1; }; done
bun $R/test/gpui/hub/sidebar_page_e2e.mjs $H 3815 > $O/result.txt 2>&1; e=$?; cat $O/result.txt; kill $HP; exit $e
