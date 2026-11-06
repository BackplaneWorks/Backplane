#!/bin/bash
R=/home/h/Documents/i2cjak/backplane-gpui; cd $R/test/gpui/perf; D=$R/build/gpui/results/perf/cmp
mkdir -p $D/run2-contaminated-idle; for f in idle latency load latencyLoad; do mv $D/$f.json $D/run2-contaminated-idle/ 2>/dev/null; done
(bun cmp_hub.mjs 3807 > $D/hub.out 2>&1 &)
sleep 8; for i in $(seq 120); do grep -q ready $D/hub.out && break; sleep 1; done
# one warm-up launch of Electron dismisses the tour (persisted in the hub)
for ph in idle latency load latencyLoad; do bun cmp_run.mjs $ph > $D/$ph.stdout 2>&1; done
touch $R/build/gpui/perf-run/cmp/stop
echo CMP3DONE; ./run_remeasure.sh
