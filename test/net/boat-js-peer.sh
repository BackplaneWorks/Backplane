#!/bin/sh
# From the repo root, on the desktop: copies the JS hub binary beside this script,
# runs the ignored two-implementation test on Boat, and removes the copy. Stand-in agents only (none), temp homes, ports 3915-3916.
set -u
BIN=${1:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/backplane-serve}
mkdir -p test/net/bin && cp "$BIN" test/net/bin/backplane-serve && chmod +x test/net/bin/backplane-serve
scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/net \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c "mkdir -p out/net; export BACKPLANE_JS_HUB=\$PWD/test/net/bin/backplane-serve; cd native/host && cargo test --bin backplane-hub net::farsync::jstests -- --ignored --nocapture --test-threads=1 > ../../out/net/js-peer.log 2>&1; rc=\$?; tail -60 ../../out/net/js-peer.log; exit \$rc"
rc=$?
rm -f test/net/bin/backplane-serve; rmdir test/net/bin 2>/dev/null
exit $rc
