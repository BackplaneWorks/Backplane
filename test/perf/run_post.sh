#!/bin/bash
R=/home/h/Documents/i2cjak/backplane-gpui; cd $R/test/perf
until [ -f $R/build/gpui/results/perf/m2/tailnet-load20-viewers20-delta1.json ]; do sleep 10; done
OUT=$R/build/gpui/results/perf/m2; export OUT
# 1. valid 20-agent hub run
bun m1_load.mjs 20 60 3801 > $R/build/gpui/results/perf/m1-20.rerun.stdout 2>&1; sleep 3
# 2. stuck readers + dropped link on a fresh loaded hub
(bun m2_server.mjs 127.0.0.1 3810 20 loop-load20b > $OUT/loop-load20b.srv 2>&1 &)
for i in $(seq 90); do [ -f $R/build/gpui/perf-run/m2-loop-load20b/state.json ] && break; sleep 1; done
ST=$R/build/gpui/perf-run/m2-loop-load20b/state.json
bun m2_client.mjs $ST 127.0.0.1 slow loopback-load20-stuck3 3 > $OUT/stuck.stdout 2>&1
bun m2_client.mjs $ST 127.0.0.1 drop loopback-load20-drop-fixed 3000 > $OUT/drop2.stdout 2>&1
bun m2_client.mjs $ST 127.0.0.1 drop loopback-load20-drop-fixed-10s 10000 > $OUT/drop3.stdout 2>&1
touch $R/build/gpui/perf-run/m2-loop-load20b/stop; sleep 4
# 3. database at scale
df -h / | tail -1 > $R/build/gpui/results/perf/m3-df.txt
for n in 10000 100000 1000000; do export EVENTS=$n;
  python3 gen_log.py $R/build/gpui/perf-run/gen/e$n.jsonl $n
  df -h / | tail -1 >> $R/build/gpui/results/perf/m3-df.txt
  bun m3_db.mjs $R/build/gpui/perf-run/gen/e$n.jsonl $n 3808 > $R/build/gpui/results/perf/m3-$n.stdout 2>&1
  rm -f $R/build/gpui/perf-run/gen/e$n.jsonl
done
rm -rf $R/build/gpui/perf-run/m3 $R/build/gpui/perf-run/gen
echo POSTDONE
