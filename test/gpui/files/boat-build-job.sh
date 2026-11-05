#!/bin/sh
# Builds backplane-hub (release) on Boat and returns the binary in out/files-bin/. Never on the desktop.
exec scripts/boat-job.sh --size default --timeout 2400 --idle 0 --no-reuse --artifact out/files-bin \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c 'mkdir -p out/files-bin; (cd native/host && cargo build --release 2>&1 | tail -60) > out/files-bin/build.txt 2>&1; cp native/host/target/release/backplane-hub out/files-bin/ 2>/dev/null; tail -40 out/files-bin/build.txt; ls -l out/files-bin'
