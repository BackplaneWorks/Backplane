#!/bin/sh
# Boat: cargo test (hub crate) then release backplane-hub; artifacts in out/pe. usage: from the repo root: sh test/gpui/wire/boat-build.sh
exec scripts/boat-job.sh --size large --timeout 2400 --idle 0 --no-reuse --artifact out/pe \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c 'mkdir -p out/pe; cd native/host && (cargo test --release --bin backplane-hub 2>&1 | tail -60 > ../../out/pe/test.txt); cargo build --release --bin backplane-hub 2>&1 | tail -40 > ../../out/pe/build.txt; cp target/release/backplane-hub ../../out/pe/ ; tail -5 ../../out/pe/test.txt'
