#!/bin/sh
# From the repo root: a release backplane-hub on Boat, delivered as the artifact out/hub/backplane-hub.
exec scripts/boat-job.sh --size default --timeout 1500 --idle 0 --no-reuse --artifact out/hub \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/hub; cd native/host && cargo build --release 2>&1 | tail -5 && cp target/release/backplane-hub ../../out/hub/ && ls -l ../../out/hub"
