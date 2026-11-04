#!/bin/sh
# quick Bend check of kernels on Boat: boat-check.sh k1 k2 ...
exec scripts/boat-job.sh --size default --timeout 600 --idle 0 --no-reuse \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' -- sh -c 'export BEND_NO_TELEMETRY=1; for k in "$@"; do echo "== $k"; bend native/bend/$k.bend --check-only 2>&1 | head -30; done' sh "$@"
