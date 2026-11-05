#!/bin/sh
# From the repo root: boat.sh KERNEL N REPS  (wraps scripts/boat-job.sh)
K=$1; N=$2; R=$3; IN=${4:-1}
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/cargo \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'stage*/**' --exclude 'native/host/target/**' --exclude 'test/gpui/perf/bin/**' --exclude 'test/gpui/hub/policy-bin/**' --exclude 'extra/**' -- sh test/gpui/bend/boat-cargo.sh $1
