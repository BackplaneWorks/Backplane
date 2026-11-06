#!/bin/sh
# Boat: the hub crate with the state policy host compiled in (main.rs has `mod shelf; mod state; #[path="state/host.rs"] mod statehost;` and statehost::start). Artifact: out/host/check.txt
set -u
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-host
O=out/host; mkdir -p $O
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "cargo check --manifest-path native/host/Cargo.toml --bin backplane-hub > $O/check.txt 2>&1; echo check-exit \$? >> $O/check.txt"
grep -E '^error' -A8 $O/check.txt | head -60; tail -3 $O/check.txt
grep -q 'check-exit 0' $O/check.txt
