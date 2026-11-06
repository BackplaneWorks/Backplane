#!/bin/sh
exec scripts/boat-job.sh --size large --timeout 1500 --idle 3600 --artifact out/sp \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' -- sh test/hub/sidebar_page_e2e.sh
