#!/bin/sh
# From the repo root: sh test/gpui/agents/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the agents module; stand-in agents only)
F=${1:-agents::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 0 --no-reuse --artifact out/agents \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/agents; cd native/host && (cargo test --bin backplane-hub $F -- --test-threads=4 --nocapture 2>&1 | tee ../../out/agents/test.log | tail -150); echo DONE >> ../../out/agents/test.log"
