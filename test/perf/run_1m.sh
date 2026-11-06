#!/bin/bash
R=/home/h/Documents/i2cjak/backplane-gpui; cd $R/test/perf
mkdir -p $R/build/gpui/perf-run/gen; df -h / | tail -1 > $R/build/gpui/results/perf/m3-df-1m.txt
export EVENTS=1000000
python3 gen_log.py $R/build/gpui/perf-run/gen/e1m.jsonl 1000000; ls -la $R/build/gpui/perf-run/gen/e1m.jsonl >> $R/build/gpui/results/perf/m3-df-1m.txt
df -h / | tail -1 >> $R/build/gpui/results/perf/m3-df-1m.txt
bun m3_db.mjs $R/build/gpui/perf-run/gen/e1m.jsonl 1000000 3808 > $R/build/gpui/results/perf/m3-1000000.stdout 2>&1
df -h / | tail -1 >> $R/build/gpui/results/perf/m3-df-1m.txt
rm -rf $R/build/gpui/perf-run/m3 $R/build/gpui/perf-run/gen
echo DONE1M
