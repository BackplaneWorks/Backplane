#!/bin/sh
exec scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/floor \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c 'mkdir -p out/floor; (cd native/host && cargo build --release > ../../out/floor/build.log 2>&1); rc=$?; tail -2 out/floor/build.log; [ $rc -eq 0 ] || exit $rc; sh test/hub/rpc_floor.sh'
