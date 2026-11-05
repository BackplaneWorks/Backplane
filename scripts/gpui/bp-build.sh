#!/bin/sh
# Provisional builds and tests on h@backplane, to find failures early. Never a gating result: anything
# that gates runs once on Boat too. One at a time, so take the lock `bp-build` through work_run.
#   scripts/gpui/bp-build.sh LANE 'cargo test --manifest-path native/host/Cargo.toml net::'
set -eu
LANE=$1; shift
REPO=$(git rev-parse --show-toplevel); D=bp-gpui-build/$LANE
COMMIT=$(git -C "$REPO" rev-parse --short HEAD)
ssh -o BatchMode=yes h@backplane "mkdir -p ~/$D"
rsync -a --delete --exclude .git --exclude build --exclude target --exclude 'native/*/target' --exclude mobile --exclude desktop \
  --exclude stage --exclude 'stage-*' --exclude .lowmem "$REPO/" "h@backplane:$D/"
echo "provisional: commit $COMMIT, dirty tree possible, host backplane"
ssh -o BatchMode=yes h@backplane "cd ~/$D && export PATH=\$HOME/.cargo/bin:\$PATH CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR=\$HOME/$D/.target && systemd-run --user --scope -q -p MemoryMax=12G -p MemorySwapMax=0 -- sh -c \"$*\""
