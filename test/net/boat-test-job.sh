#!/bin/sh
# From the repo root: sh test/net/boat-test-job.sh [FILTER]   (a Boat sandbox: cargo test of the net module; stand-in agents only)
F=${1:-net::}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/net \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c "mkdir -p out/net; cd native/host && cargo test --bin backplane-hub $F -- --test-threads=4 --nocapture > ../../out/net/test.log 2>&1; rc=\$?; tail -150 ../../out/net/test.log; echo DONE rc=\$rc >> ../../out/net/test.log; exit \$rc"
