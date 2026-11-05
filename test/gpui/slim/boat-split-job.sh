#!/bin/sh
# From the repo root: the slim policy via the split build on Boat (about 10 min). Artifact out/split/bp-policy.stripped
exec scripts/boat-job.sh --size xlarge --timeout 2400 --idle 0 --no-reuse --artifact out/split \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh test/gpui/slim/build-split.sh
