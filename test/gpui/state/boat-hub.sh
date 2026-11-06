#!/bin/sh
# Boat: the hub crate with the state policies compiled in, and the state:: tests of the hub binary. Artifact: out/hub/test.txt
set -u
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-hub
O=out/hub; mkdir -p $O
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "cargo test --release --manifest-path native/host/Cargo.toml --bin backplane-hub state:: > $O/test.txt 2>&1; echo test-exit \$? >> $O/test.txt"
grep -E '^error' -A8 $O/test.txt | head -30; grep -E 'test result|test-exit' $O/test.txt
grep -q 'test-exit 0' $O/test.txt
