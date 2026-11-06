#!/bin/sh
# From the repo root: builds nothing; builds backplane-hub first via boat-cargo, then runs fold_scale.sh on Boat.
# policy-bin/ holds a copy of build/gpui/slim/bp-policy-serve-O2 (untracked, never committed).
exec scripts/boat-job.sh --size default --timeout 3000 --idle 3600 --artifact out/fold \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' -- sh -c 'mkdir -p out/fold; (cd native/host && cargo build --release > ../../out/fold/build.log 2>&1); rc=$?; tail -2 out/fold/build.log; [ $rc -eq 0 ] || exit $rc; sh test/gpui/hub/fold_scale.sh'
