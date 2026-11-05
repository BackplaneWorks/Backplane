#!/bin/bash
# seeded-hub churn (hub), GPUI churn with reconnects, 2-hour GPUI soak, one after another; detached
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf; D=../../../build/gpui/results/perf
bun m4_churn.mjs 20 3806 > $D/m4-churn.stdout 2>&1; echo hubchurn rc=$? >> $D/final.status
sleep 5
bun cmp_churn.mjs 20 > $D/gpui-churn.stdout 2>&1; echo gpuichurn rc=$? >> $D/final.status
sleep 5
mkdir -p $D/gpui-soak-short; mv $D/gpui-soak/samples.jsonl $D/gpui-soak-short/ 2>/dev/null
bun cmp_soak.mjs 120 > $D/gpui-soak.stdout 2>&1; echo gpuisoak rc=$? >> $D/final.status
echo FINALDONE >> $D/final.status
