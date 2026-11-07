#!/bin/sh
cd "$(dirname "$0")"
for n in 1 10 20 50; do bun m1_load.mjs $n 60 > $(git -C "$(dirname "$0")" rev-parse --show-toplevel)/build/gpui/results/perf/m1-$n.stdout 2>&1; sleep 3; done
echo done
