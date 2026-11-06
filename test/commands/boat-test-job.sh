#!/bin/sh
# From the repo root: sh test/commands/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the commands module; fixtures only)
F=${1:-commands::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/commands \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c "mkdir -p out/commands; cd native/host && cargo test --bin backplane-hub $F -- --test-threads=4 --nocapture > ../../out/commands/test.log 2>&1; rc=\$?; tail -150 ../../out/commands/test.log; echo DONE rc=\$rc >> ../../out/commands/test.log; exit \$rc"
