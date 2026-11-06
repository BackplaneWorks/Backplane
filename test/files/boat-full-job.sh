#!/bin/sh
# Runs the files-stream tests on Boat, never on the desktop: the harness crate of pure modules, then the hub crate's files:: tests.
# Full output: out/full/tests.txt and crate-tests.txt. Exits non-zero when either cargo does (it used to end in tail, which hid a failed compile).
exec scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/full \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'test/perf/bin/**' --exclude 'test/parity/*.tgz' --exclude 'test/slim/*.gz' -- sh -c 'mkdir -p out/full; (cd test/files/harness && cargo test) > out/full/tests.txt 2>&1; h=$?; (cd native/host && cargo test) > out/full/crate-tests.txt 2>&1; c=$?; grep -n -A14 "^error" out/full/crate-tests.txt | head -120; grep -n -A14 "^error" out/full/tests.txt | head -60; grep -E "^test result|FAILED|panicked" out/full/tests.txt out/full/crate-tests.txt | tail -40; echo "HARNESS_EXIT=$h CRATE_EXIT=$c"; [ "$h" = 0 ] && [ "$c" = 0 ]'
