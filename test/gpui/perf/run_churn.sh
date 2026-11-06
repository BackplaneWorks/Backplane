#!/bin/bash
while [ -d /proc/3037444 ]; do sleep 15; done
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf && bun m4_churn.mjs 20 3806 > ../../../build/gpui/results/perf/m4-churn.stdout 2>&1; echo CHURNDONE
