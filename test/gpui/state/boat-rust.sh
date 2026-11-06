#!/bin/sh
# Boat: unit tests of the state fold, then rust-digest --json on every synthetic log in test/gpui/state/logs. Artifacts: out/state/*.json, test.txt
set -u
O=out/state; mkdir -p $O
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-state
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "
  cargo test --release --manifest-path native/host/Cargo.toml --bin rust-digest > $O/test.txt 2>&1; echo test-exit \$? >> $O/test.txt
  cargo build --release --manifest-path native/host/Cargo.toml --bin rust-digest > $O/build.txt 2>&1; echo build-exit \$? >> $O/build.txt
"
tail -30 $O/test.txt; tail -3 $O/build.txt
for f in test/gpui/state/logs/*.jsonl; do n=$(basename $f .jsonl); target-state/release/rust-digest --json $f > $O/$n.json 2> $O/$n.err; done
grep -q "test-exit 0" $O/test.txt && grep -q "build-exit 0" $O/build.txt
