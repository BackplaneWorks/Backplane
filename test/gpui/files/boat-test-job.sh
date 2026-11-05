#!/bin/sh
# Runs the files-stream pure-module tests (harness crate) on Boat, never on the desktop. Output: out/files/tests.txt
exec scripts/boat-job.sh --size default --timeout 1800 --idle 0 --no-reuse --artifact out/files \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c 'mkdir -p out/files; (cd test/gpui/files/harness && cargo test 2>&1 | tail -120) > out/files/tests.txt 2>&1; (cd native/host && cargo test kicad:: 2>&1 | tail -40; cargo test files:: 2>&1 | tail -30) > out/files/crate-tests.txt 2>&1; tail -25 out/files/crate-tests.txt; tail -70 out/files/tests.txt'
