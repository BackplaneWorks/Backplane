#!/bin/sh
# Runs the files-stream tests on Boat, never on the desktop: the harness crate of pure modules, then the hub crate's files:: tests.
# Full output: out/files/tests.txt and crate-tests.txt. Exits non-zero when either cargo does (it used to end in tail, which hid a failed compile).
exec scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/files \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'test/gpui/perf/bin/**' --exclude 'test/gpui/parity/*.tgz' --exclude 'test/gpui/slim/*.gz' -- sh -c 'mkdir -p out/files; (cd test/gpui/files/harness && cargo test) > out/files/tests.txt 2>&1; h=$?; (cd native/host && cargo test files::) > out/files/crate-tests.txt 2>&1; c=$?; grep -n -A14 "^error" out/files/crate-tests.txt | head -120; grep -n -A14 "^error" out/files/tests.txt | head -60; grep -E "^test result|FAILED|panicked" out/files/tests.txt out/files/crate-tests.txt | tail -40; echo "HARNESS_EXIT=$h CRATE_EXIT=$c"; [ "$h" = 0 ] && [ "$c" = 0 ]'
