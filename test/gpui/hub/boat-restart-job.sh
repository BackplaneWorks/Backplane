#!/bin/sh
exec scripts/boat-job.sh --size default --timeout 1800 --idle 0 --no-reuse --artifact out/restart \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c '(cd native/host && cargo build --release 2>&1 | tail -2) && sh test/gpui/hub/restart_scale.sh'
