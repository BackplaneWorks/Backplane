#!/bin/sh
# Provisional builds and tests on h@backplane, to find failures early. Never a gating result: anything
# that gates runs once on Boat too. Three slots of 5 GB and 5 cores (scripts/gpui/bp-slot.sh): take NO lock, and run focused tests.
#   scripts/gpui/bp-build.sh LANE 'cargo test --manifest-path native/host/Cargo.toml net::'
set -eu
LANE=$1; shift
REPO=$(git rev-parse --show-toplevel); D=bp-gpui-build/$LANE
COMMIT=$(git -C "$REPO" rev-parse --short HEAD)
# The tree is the worktree you're standing in, not the script's: a lane's run from the main checkout tests the wrong code.
case "$REPO" in */backplane-gpui) [ "$LANE" = hubfull ] || [ "${BP_BUILD_MAIN:-}" = 1 ] || { echo "bp-build: cd into your lane's worktree first (this is the main checkout)" >&2; exit 64; } ;; esac
ssh -o BatchMode=yes h@backplane "mkdir -p ~/$D"
rsync -a --delete --exclude .git --exclude build --exclude target --exclude 'native/*/target' --exclude mobile --exclude desktop \
  --exclude stage --exclude 'stage-*' --exclude .lowmem --exclude .target "$REPO/" "h@backplane:$D/"
echo "provisional: commit $COMMIT, tree $REPO, dirty tree possible, host backplane"
ssh -o BatchMode=yes h@backplane "sh ~/$D/scripts/gpui/bp-slot.sh $LANE $(printf %s "$*" | base64 -w0)"
