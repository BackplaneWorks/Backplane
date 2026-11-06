#!/bin/sh
# Builds backplane-hub (release) on Boat and returns the binary in out/files-bin/. Never on the desktop.
exec scripts/boat-job.sh --size default --timeout 2400 --idle 3600 --artifact out/files-bin \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'test/perf/bin/**' --exclude 'test/parity/*.tgz' --exclude 'test/slim/*.gz' -- sh -c 'mkdir -p out/files-bin; (cd native/host && cargo build --release --bin backplane-hub 2>&1) > out/files-bin/build.txt 2>&1; c=$?; [ $c = 0 ] && cp native/host/target/release/backplane-hub out/files-bin/; grep -n -A24 "^error" out/files-bin/build.txt | head -120; tail -3 out/files-bin/build.txt; ls -l out/files-bin; exit $c'
