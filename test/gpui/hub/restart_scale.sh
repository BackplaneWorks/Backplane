#!/bin/sh
# restart_scale.sh : BOAT ONLY. Synthetic 1M-event log: time from process start to first usable view
# (listening + a view.open answered), RSS, then durable-write latency and RSS growth per committed event.
case "$HOME" in /home/h|/home/h/*) echo "restart_scale.sh refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "restart_scale.sh: not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; OUT=$R/out/restart; mkdir -p $OUT; HUB=$R/native/host/target/release/backplane-hub
H=$OUT/home; rm -rf $H; mkdir -p $H
python3 - > $H/events.jsonl <<'PY'
import sys, json
out = sys.stdout; t0 = 1785000000; n = 1000000; threads = 3000
for p in range(60): out.write(json.dumps({"$":"ProjectCreated","id":f"p{p}","title":f"proj {p}","root":f"/tmp/p{p}","at":t0}) + "\n")
for t in range(threads): out.write(json.dumps({"$":"ThreadCreated","id":f"t{t}","project":f"p{t%60}","title":f"Thread {t}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t0+t}) + "\n")
for i in range(n - 60 - threads):
    t = (i * 7919) % threads
    if i % 5 == 4: out.write(json.dumps({"$":"ActivityLogged","thread":f"t{t}","act":f"a{i}","tone":"tool","kind":"Bash","summary":"ls -la src/core | head","at":t0+i}) + "\n")
    else: out.write(json.dumps({"$":"MessagePosted","thread":f"t{t}","msg":f"m{i}","role":"user" if i % 2 else "assistant","text":"a message of some length, as a person or a model would write it, " * 3,"at":t0+i}) + "\n")
PY
echo "log: $(wc -l < $H/events.jsonl) lines, $(stat -c %s $H/events.jsonl) bytes" | tee $OUT/restart.txt
sync; echo 3 > /proc/sys/vm/drop_caches 2>/dev/null   # cold cache when allowed
T0=$(date +%s.%N)
$HUB --home $H --port 3815 > $H/log 2>&1 & HP=$!
while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.05; done
T1=$(date +%s.%N)
echo "restart: listening after $(echo "$T1 - $T0" | bc) s, hub rss_kb $(awk '/VmRSS/{print $2}' /proc/$HP/status) hwm_kb $(awk '/VmHWM/{print $2}' /proc/$HP/status)" | tee -a $OUT/restart.txt
bun $R/test/gpui/hub/rss_growth.mjs $H 3815 $HP ${WRITES:-8000} | tee -a $OUT/restart.txt
kill $HP; sleep 1
T2=$(date +%s.%N); $HUB --home $H --port 3815 > $H/log 2>&1 & HP=$!
while ! curl -s -m 1 http://127.0.0.1:3815/hello > /dev/null; do sleep 0.05; done
echo "restart after writes ($(wc -l < $H/events.jsonl) lines): listening after $(echo "$(date +%s.%N) - $T2" | bc) s" | tee -a $OUT/restart.txt
kill $HP; rm -rf $H
