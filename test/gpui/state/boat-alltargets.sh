#!/bin/sh
set -u
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-all
mkdir -p out/all
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "cargo test --release --manifest-path native/host/Cargo.toml --no-run --all-targets > out/all/build.txt 2>&1; echo build-exit \$? >> out/all/build.txt"
grep -E '^error' -A7 out/all/build.txt | head -30; tail -3 out/all/build.txt
grep -q 'build-exit 0' out/all/build.txt
