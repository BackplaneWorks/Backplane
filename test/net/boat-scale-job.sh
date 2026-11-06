#!/bin/sh
# From the repo root: a 1M-line far mirror's start time and memory on Boat (synthetic data only).
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/net \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c "mkdir -p out/net; cd native/host && cargo test --release --bin backplane-hub net::farstore::tests::scale_a_million_line_mirror -- --ignored --nocapture --test-threads=1 > ../../out/net/scale.log 2>&1; rc=\$?; grep -E 'SCALE|panicked|test result' ../../out/net/scale.log; exit \$rc"
