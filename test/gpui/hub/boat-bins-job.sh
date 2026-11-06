#!/bin/sh
exec scripts/boat-job.sh --size large --timeout 1500 --idle 3600 --artifact out/bins \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'stage*/**' --exclude 'native/host/target/**' --exclude 'test/gpui/perf/bin/**' --exclude 'test/gpui/hub/policy-bin/**' --exclude 'extra/**' -- sh test/gpui/hub/bins.sh
