#!/bin/bash
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf; D=../../../build/gpui/results/perf
rm -f $D/m0/m0.json
bun m0_baseline.mjs later > $D/m0.later.stdout 2>&1; echo later rc=$?
tail -c 600 $D/m0.later.stdout
echo M0DDONE
