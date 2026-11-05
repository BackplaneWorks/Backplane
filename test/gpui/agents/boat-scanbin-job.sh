#!/bin/sh
# From the repo root: build (not run) the agents test binary on Boat and fetch it; run it here against the store copy with a memory cap.
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/scan \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/scan; cd native/host && cargo test --release --no-run --message-format=json > ../../out/scan/cargo.json 2>/dev/null; rc=\$?; grep -o '\"executable\":\"[^\"]*backplane_hub-[^\"]*\"' ../../out/scan/cargo.json | head -1 | cut -d'\"' -f4 | xargs -I{} cp {} ../../out/scan/hubtest; [ -s ../../out/scan/hubtest ] || rc=1; exit \$rc"
