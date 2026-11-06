#!/bin/sh
# fold_scale.sh : time to listening and memory of backplane-hub + bp-policy folding the first N events of a
# SYNTHETIC log. BOAT ONLY: it refuses to run anywhere else (COMMON.md: nothing heavy on the desktop; the
# store copy never leaves it). Run through test/hub/boat-fold-job.sh. Hard cap 3.7 GiB, no swap.
case "$HOME" in /home/h|/home/h/*) echo "fold_scale.sh refuses to run on the desktop: use test/hub/boat-fold-job.sh" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "fold_scale.sh: not a Boat sandbox (user $(id -un)); refusing" >&2; exit 3; }
R=$PWD
OUT=$R/out/fold; mkdir -p $OUT; : > $OUT/fold-scale.txt
HUB=$R/native/host/target/release/backplane-hub; POL=$R/test/hub/policy-bin/bp-policy-serve-O2
CAP=${CAP:-3700000000}
for N in ${SIZES:-25000 100000 200000 400000 800000}; do
  H=$R/out/fold/home-$N; rm -rf $H; mkdir -p $H
  # synthetic: 50 projects, 1 thread per 20 events, messages/activity spread over threads (deterministic)
  python3 - $N > $H/events.jsonl <<'PY'
import sys, json
n = int(sys.argv[1]); out = sys.stdout; t0 = 1785000000; threads = max(10, n // 20)
for p in range(50): out.write(json.dumps({"$":"ProjectCreated","id":f"p{p}","title":f"proj {p}","root":f"/tmp/p{p}","at":t0}) + "\n")
for t in range(threads): out.write(json.dumps({"$":"ThreadCreated","id":f"t{t}","project":f"p{t%50}","title":f"Thread {t}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t0+t}) + "\n")
for i in range(max(0, n - 50 - threads)):
    t = (i * 7919) % threads
    if i % 5 == 4: out.write(json.dumps({"$":"ActivityLogged","thread":f"t{t}","act":f"a{i}","tone":"tool","kind":"Bash","summary":"ls -la src/core | head","at":t0+i}) + "\n")
    else: out.write(json.dumps({"$":"MessagePosted","thread":f"t{t}","msg":f"m{i}","role":"user" if i % 2 else "assistant","text":"a message of some length, as a person or a model would write it, " * 3,"at":t0+i}) + "\n")
PY
  echo "N=$N lines=$(wc -l < $H/events.jsonl) bytes=$(stat -c %s $H/events.jsonl)" >> $OUT/fold-scale.txt
  # hard cap: one cgroup-less guard, ulimit on address space is too blunt for , so poll RSS of the whole tree
  $HUB --home $H --port 3815 --policy-bin $POL > $H/log 2>&1 &
  HP=$!; T0=$(date +%s.%N); OK=0; PEAK=0; DIED=""
  while :; do
    sleep 0.2
    kill -0 $HP 2>/dev/null || { DIED=exited; break; }
    TREE=$(ps -o rss= --ppid $HP -p $HP | awk '{s+=$1} END{print s*1024}')
    [ "${TREE:-0}" -gt "$PEAK" ] && PEAK=$TREE
    [ "${TREE:-0}" -gt "$CAP" ] && { DIED=capped; kill -9 $(pgrep -P $HP) $HP 2>/dev/null; break; }
    curl -s -m 1 http://127.0.0.1:3815/hello >/dev/null && { OK=1; break; }
    [ $(echo "$(date +%s.%N) - $T0 > 600" | bc) = 1 ] && { DIED=timeout; break; }
  done
  T1=$(date +%s.%N)
  PP=$(pgrep -P $HP | head -1)
  echo "N=$N listening=$OK ${DIED:+stopped=$DIED} seconds=$(echo "$T1 - $T0" | bc) tree_peak_mb=$((PEAK/1048576)) hub_rss_kb=$(awk '/VmRSS/{print $2}' /proc/$HP/status 2>/dev/null) policy_rss_kb=$(awk '/VmRSS/{print $2}' /proc/$PP/status 2>/dev/null)" >> $OUT/fold-scale.txt
  kill $HP 2>/dev/null; sleep 1; rm -rf $H
done
cat $OUT/fold-scale.txt
