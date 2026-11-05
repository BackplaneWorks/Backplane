#!/bin/sh
exec scripts/boat-job.sh --size large --timeout 1500 --idle 3600 --artifact out/we \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/bp-policy-serve-O2' -- sh test/gpui/hub/window_e2e.sh
