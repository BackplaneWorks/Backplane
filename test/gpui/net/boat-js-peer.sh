#!/bin/sh
# From the repo root, on the desktop: copies the JS hub binary (build/gpui/backplane-serve, a Bend hub built by the Bend stream) beside this script,
# runs the ignored two-implementation test on Boat, and removes the copy. Stand-in agents only (none), temp homes, ports 3915-3916.
set -u
BIN=${1:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/backplane-serve}
mkdir -p test/gpui/net/bin && cp "$BIN" test/gpui/net/bin/backplane-serve && chmod +x test/gpui/net/bin/backplane-serve
scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --no-setup-bend --artifact out/net \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c "mkdir -p out/net; export BACKPLANE_JS_HUB=\$PWD/test/gpui/net/bin/backplane-serve; cd native/host && cargo test --bin backplane-hub net::farsync::jstests -- --ignored --nocapture --test-threads=1 > ../../out/net/js-peer.log 2>&1; rc=\$?; tail -60 ../../out/net/js-peer.log; exit \$rc"
rc=$?
rm -f test/gpui/net/bin/backplane-serve; rmdir test/gpui/net/bin 2>/dev/null
exit $rc
