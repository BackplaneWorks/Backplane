#!/bin/sh
exec scripts/boat-job.sh --size large --timeout 1500 --idle 3600 --artifact out/sh \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'stage*/**' --exclude 'native/host/target/**' --exclude 'test/perf/bin/**' --exclude 'test/hub/policy-bin/**' --exclude 'extra/**' -- sh test/hub/shown_e2e.sh
