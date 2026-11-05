#!/bin/sh
# BOAT ONLY: build the Rust hub from this tree, then run the private-lines scenario against it (boat-private.sh with $STAGE/hub = the new binary).
set -u
STAGE=${STAGE:-stage}
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p out; (cd native/host && rm -f target/release/backplane-hub && systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- cargo build --release --bin backplane-hub > ../../out/hubbuild.log 2>&1); bh=$?; grep -E '^error|Finished' out/hubbuild.log | head -8; [ $bh -eq 0 ] || exit $bh
H=${CARGO_TARGET_DIR:-native/host/target}/release/backplane-hub
[ -x $H ] || { echo "no hub at $H"; exit 3; }
cp $H $STAGE/hub
exec sh test/gpui/parity/boat-private.sh
