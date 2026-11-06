#!/bin/bash
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf; D=../../../build/gpui/results/perf
cp $D/m0/m0.json $D/m0/m0.before-load-fix.json
bun m0_baseline.mjs load > $D/m0.load.stdout 2>&1; echo load rc=$?; tail -c 400 $D/m0.load.stdout
echo M0FDONE
