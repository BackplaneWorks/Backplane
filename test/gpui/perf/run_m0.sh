#!/bin/bash
# waits for my other runs so the baseline is measured on a quiet machine
for p in 346958 2865009 2920909; do while [ -d /proc/$p ]; do sleep 15; done; done
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf
df -h / | tail -1 > ../../../build/gpui/results/perf/m0-df.txt
bun m0_baseline.mjs all > ../../../build/gpui/results/perf/m0.stdout 2>&1
echo M0DONE
