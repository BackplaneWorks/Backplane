#!/bin/sh
# From the repo root: sh test/gpui/agents/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the agents module; stand-in agents only)
F=${1:-agents::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/agents \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/agents; cd native/host && cargo test --bin backplane-hub $F -- --test-threads=2 --nocapture > ../../out/agents/test.log 2>&1; rc=\$?; tail -150 ../../out/agents/test.log; echo DONE rc=\$rc >> ../../out/agents/test.log; exit \$rc"
