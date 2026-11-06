#!/bin/sh
# rpc_floor.sh : BOAT ONLY. Ping and write-ack numbers of backplane-hub (no presenter).
case "$HOME" in /home/h|/home/h/*) echo "rpc_floor.sh refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "rpc_floor.sh: not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; OUT=$R/out/floor; mkdir -p $OUT; H=$OUT/home; rm -rf $H; mkdir -p $H
python3 - > $H/events.jsonl <<'PY'
import json, sys
o = sys.stdout
o.write(json.dumps({"$":"ProjectCreated","id":"p0","title":"p","root":"/tmp/p","at":1785000000}) + "\n")
for t in range(50): o.write(json.dumps({"$":"ThreadCreated","id":f"t{t}","project":"p0","title":f"T{t}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1785000000}) + "\n")
PY
$R/native/host/target/release/backplane-hub --home $H --port 3815 > $H/log 2>&1 & HP=$!
while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.05; done
echo "machine: $(nproc) vCPU, $(uname -m); disk fsync is what the write ack waits for" | tee $OUT/floor.txt
bun $R/test/hub/rpc_floor.mjs $H 3815 | tee -a $OUT/floor.txt
kill $HP
