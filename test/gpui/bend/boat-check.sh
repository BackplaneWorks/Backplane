#!/bin/sh
# quick Bend check of kernels on Boat: boat-check.sh k1 k2 ...
exec scripts/boat-job.sh --size default --timeout 600 --idle 3600 \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' -- sh -c 'export BEND_NO_TELEMETRY=1; . scripts/bendok.sh; rc=0; for k in "$@"; do echo "== $k"; bend_check native/bend/$k.bend || rc=1; done; exit $rc' sh "$@"
