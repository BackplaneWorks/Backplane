#!/bin/bash
cd /home/h/Documents/i2cjak/backplane-gpui/test/perf; D=../../../build/gpui/results/perf
for ph in steady load; do bun m0_baseline.mjs $ph > $D/m0.$ph.stdout 2>&1; echo $ph rc=$?; tail -c 300 $D/m0.$ph.stdout | head -c 300; done
echo M0EDONE
