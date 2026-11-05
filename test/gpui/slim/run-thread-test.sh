#!/bin/sh
# on h@backplane via scripts/gpui/bp-build.sh: the thread view's focused tests
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path native/host/Cargo.toml --bin backplane-hub present:: 2>&1 | grep -E '^error|test result|FAILED|panicked|left:|right:|^test ' -A3 | cut -c1-300 | head -60
