#!/bin/sh
set -u
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-host2
mkdir -p out/hostt
cargo test --release --manifest-path native/host/Cargo.toml --bin backplane-hub statehost:: > out/hostt/test.txt 2>&1; echo "test-exit $?" >> out/hostt/test.txt
grep -E '^test |test result|panicked|missing' -A3 out/hostt/test.txt | head -30
grep -q 'test-exit 0' out/hostt/test.txt
