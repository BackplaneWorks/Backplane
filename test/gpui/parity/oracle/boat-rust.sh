#!/bin/sh
# Boat: build rust-digest (the Rust side of the oracle) and run the state.rs unit tests. Artifacts: out/oracle/rust-digest, test.txt
set -u
O=out/oracle; mkdir -p $O
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-oracle
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "
  /usr/bin/time -f 'cargo test: %e s %M KiB' cargo test --release --manifest-path native/host/Cargo.toml --bin rust-digest --bin sidebar-check > $O/test.txt 2>&1; echo test-exit \$? >> $O/test.txt
  /usr/bin/time -f 'cargo build: %e s %M KiB' cargo build --release --manifest-path native/host/Cargo.toml --bin rust-digest --bin sidebar-check > $O/build.txt 2>&1; echo build-exit \$? >> $O/build.txt
"
cp target-oracle/release/rust-digest target-oracle/release/sidebar-check $O/ 2>/dev/null; tail -15 $O/test.txt; tail -5 $O/build.txt; ls -l $O/rust-digest
grep -q "test-exit 0" $O/test.txt && grep -q "build-exit 0" $O/build.txt
