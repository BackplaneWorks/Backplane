#!/bin/bash
true
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel); cd $R/test/perf; D=$R/build/gpui/results/perf
mkdir -p $D/superseded-20ms-poll; for n in 1 10 20 50; do mv $D/m1-${n}agents $D/superseded-20ms-poll/ 2>/dev/null; done
bun remeasure_rpc.mjs $D/rpc-eventdriven.json $R/build/gpui/backplane-serve $R/build/gpui/files-bin/backplane-hub > $D/rpc-eventdriven.stdout 2>&1
for n in 1 10 20 50; do bun m1_load.mjs $n 60 3801 > $D/m1-$n.rerun2.stdout 2>&1; sleep 3; done
echo REMEASUREDONE
