#!/bin/sh
# From the repo root: boat-job.sh proof SECS | crash SCENARIO ROUNDS [EVENTS]. Artifact: out/soak.
mode=$1; shift
case $mode in proof) T=$(( ${1:-3600} + 900 )) ;; hop) T=${BOAT_T:-6600} ;; *) T=${BOAT_T:-3000} ;; esac
exec scripts/boat-job.sh --size default --timeout $T --idle 0 --no-reuse --artifact out/soak \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh test/soak/in-boat.sh $mode "$@"
