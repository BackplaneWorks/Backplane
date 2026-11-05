#!/bin/sh
# From the repo root: sh test/gpui/term/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the term lane; no agents, no model)
F=${1:-term::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 0 --no-reuse --artifact out/term \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/term; cd native/host && (cargo test --bin backplane-hub $F -- --test-threads=4 --nocapture 2>&1 | tee ../../out/term/test.log | tail -150); echo DONE >> ../../out/term/test.log"
