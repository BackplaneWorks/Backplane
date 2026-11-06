#!/bin/sh
exec scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/restart \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c 'mkdir -p out/restart; (cd native/host && cargo build --release > ../../out/restart/build.log 2>&1); rc=$?; tail -2 out/restart/build.log; [ $rc -eq 0 ] || exit $rc; sh test/hub/restart_scale.sh'
