#!/bin/bash
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf
mkdir -p ../../../build/gpui/results/perf/m0
rm -f ../../../build/gpui/results/perf/m0/m0.json
df -h / | tail -1 > ../../../build/gpui/results/perf/m0-df.txt
(bun m0_watch.mjs &) 
bun m0_baseline.mjs first > ../../../build/gpui/results/perf/m0.first.stdout 2>&1
echo FIRSTDONE rc=$?
