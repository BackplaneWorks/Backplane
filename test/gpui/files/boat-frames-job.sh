#!/bin/sh
# Boat: writes the real first plot frames of the ecc83 demo (board, schematic) to out/frames/ (copy them to fixtures/plot/).
exec scripts/boat-job.sh --size default --timeout 1200 --idle 3600 --artifact out/frames \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c '
mkdir -p out/frames; (cd test/gpui/files/harness && FRAME_OUT=$PWD/../../../../out/frames cargo test --test frames -- --nocapture 2>&1 | tail -8) | tee out/frames/run.txt; ls -l out/frames'
