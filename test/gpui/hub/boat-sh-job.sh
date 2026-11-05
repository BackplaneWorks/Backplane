#!/bin/sh
exec scripts/boat-job.sh --size large --timeout 1500 --idle 0 --no-reuse --artifact out/sh \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' -- sh test/gpui/hub/shown_e2e.sh
