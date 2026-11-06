#!/bin/sh
# From the repo root: sh test/gpui/bots/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the bots module; stand-in agents only)
# Each test line streams as it ends (grep --line-buffered) and the run is capped, so a hung test shows which one.
F=${1:-bots::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/bots \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/bots; cd native/host && timeout 900 cargo test --bin backplane-hub $F -- --test-threads=4 --nocapture > ../../out/bots/test.log 2>&1; rc=\$?; grep -E '^error|^test |panicked|test result|^  *--> src/bots' ../../out/bots/test.log; echo DONE rc=\$rc; exit \$rc"
