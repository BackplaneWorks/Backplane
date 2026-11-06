#!/bin/sh
# Build hub and window (fast profile) on h@backplane and copy the two binaries here: bp-fetch.sh LANE OUTDIR
# The window build is online: native/gpui's lockfile names the optional `cef` crate, which the builder's offline registry lacks.
set -eu
LANE=$1; OUT=$2; HERE=$(cd "$(dirname "$0")" && pwd)
"$HERE/bp-build.sh" "$LANE" 'cargo build --profile fast --manifest-path native/host/Cargo.toml --bin backplane-hub 2>&1 | tail -3; cargo build --profile fast --manifest-path native/gpui/Cargo.toml 2>&1 | tail -3'
mkdir -p "$OUT"
T=bp-gpui-build/$LANE/.target/fast
rsync -a "h@backplane:$T/backplane-hub" "h@backplane:$T/backplane-gpui" "$OUT/"
ls -l "$OUT"
