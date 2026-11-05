#!/bin/sh
# From the repo root, ONE Boat job (a warm sandbox, one start): build the policy (split build), run policy_local.mjs against it on a generated
# synthetic log, then policy_compact.mjs. Artifacts out/split (the binary) and out/pl (results).
exec scripts/boat-job.sh --size xlarge --timeout 2700 --idle 3600 --artifact out/split --artifact out/pl \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' --exclude 'shots-snap/**' \
  --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' --exclude 'skills/**' --exclude 'stage*/**' \
  --exclude 'native/host/target/**' --exclude 'test/gpui/perf/bin/**' --exclude 'test/gpui/hub/policy-bin/**' --exclude 'extra/**' \
  -- sh -c 'sh test/gpui/slim/build-split.sh && sh test/gpui/slim/policy-checks.sh'
