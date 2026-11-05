#!/bin/bash
while [ -d /proc/545091 ]; do sleep 10; done
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf; D=../../../build/gpui/results/perf/cmp
mkdir -p $D/run1-invalid-electron; for f in idle latency load latencyLoad; do mv $D/$f.json $D/run1-invalid-electron/ 2>/dev/null; done
for ph in idle latency load latencyLoad; do bun cmp_run.mjs $ph > $D/$ph.stdout 2>&1; done
echo CMP2DONE
