#!/bin/sh
# KiCad IPC in Rust, on Boat, never on the desktop. The probe crate compiles the transport alone (kipc.rs, nng.rs, proto.rs), so its
# tests and binary do not depend on the rest of the hub building; then the hub's own `cargo test kicad::`. The binary has the Bun
# helper's NDJSON interface, for the desktop's checks against the real server. Output: out/kipc/{probe-tests,probe,tests}.txt, kipc-probe.
# Exit 0 only when the probe's tests pass and the binary exists; the hub's result is printed as HUB_TESTS=<code>, since other
# streams' unfinished work in the shared tree can stop that build.
exec scripts/boat-job.sh --size default --timeout 2400 --idle 3600 --artifact out/kipc \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c '
mkdir -p out/kipc; here=$PWD
(cd test/gpui/files/kipc-probe && cargo test > "$here/out/kipc/probe-tests.txt" 2>&1); t=$?
(cd test/gpui/files/kipc-probe && cargo build --release > "$here/out/kipc/probe.txt" 2>&1); b=$?
cp test/gpui/files/kipc-probe/target/release/kipc-probe out/kipc/ 2>/dev/null
(cd native/host && cargo test kicad:: > "$here/out/kipc/tests.txt" 2>&1); h=$?
tail -6 out/kipc/probe.txt; tail -50 out/kipc/probe-tests.txt; tail -8 out/kipc/tests.txt; ls -l out/kipc
echo "PROBE_TESTS=$t PROBE_BUILD=$b HUB_TESTS=$h"
[ "$t" = 0 ] && [ "$b" = 0 ] && [ -x out/kipc/kipc-probe ]'
