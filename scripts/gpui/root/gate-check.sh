#!/bin/sh
# The gate-metadata batch: no whole-crate test execution. Run through bp-build.sh root.
cd native/host || exit 1
cargo test --release --no-run --all-targets > /tmp/bp-root-gate-compile.log 2>&1
rc=$?
printf 'all-target compile exit code: %s\n' "$rc"
if [ "$rc" != 0 ]; then cat /tmp/bp-root-gate-compile.log; exit "$rc"; fi
cargo test --release --bin backplane-hub -- state:: > /tmp/bp-root-gate-state.log 2>&1
rc=$?
printf 'state tests exit code: %s\n' "$rc"
if [ "$rc" != 0 ]; then cat /tmp/bp-root-gate-state.log; exit "$rc"; fi
sed -n '/^running /p; /^test result:/p' /tmp/bp-root-gate-state.log
cargo test --release --bin backplane-hub -- statehost:: > /tmp/bp-root-gate-statehost.log 2>&1
rc=$?
printf 'statehost tests exit code: %s\n' "$rc"
if [ "$rc" != 0 ]; then cat /tmp/bp-root-gate-statehost.log; exit "$rc"; fi
sed -n '/^running /p; /^test result:/p' /tmp/bp-root-gate-statehost.log
