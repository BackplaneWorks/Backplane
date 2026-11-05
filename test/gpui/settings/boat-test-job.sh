#!/bin/sh
# From the repo root: sh test/gpui/agents/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the agents module; stand-in agents only)
F=${1:-settings::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/settings \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/settings; cd native/host && (cargo test --bin backplane-hub $F -- --test-threads=4 --nocapture 2>&1 | tee ../../out/settings/test.log | tail -150); echo DONE >> ../../out/settings/test.log"
