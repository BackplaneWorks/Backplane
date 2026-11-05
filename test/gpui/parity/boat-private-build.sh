#!/bin/sh
# BOAT ONLY: build the Rust hub from this tree, then run the private-lines scenario against it (boat-private.sh with stage/hub = the new binary).
set -u
export PATH="$HOME/.cargo/bin:$PATH"
(cd native/host && systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- cargo build --release --bin backplane-hub 2>&1 | grep -E '^error|Finished' | head -8)
[ -x native/host/target/release/backplane-hub ] || exit 3
cp native/host/target/release/backplane-hub stage/hub
exec sh test/gpui/parity/boat-private.sh
