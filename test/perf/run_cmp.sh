#!/bin/bash
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"/test/perf
for ph in startup idle latency load latencyLoad; do bun cmp_run.mjs $ph > ../../../build/gpui/results/perf/cmp/$ph.stdout 2>&1; done
echo CMPDONE
