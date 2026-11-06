#!/bin/sh
# Sync native/gpui to the Mac and build there. Never compiles locally.
# usage: scripts/mac-build.sh [cold|check] [extra cargo args...]
# Takes the Mac-side lock itself: lockf -k /tmp/bp-mac-heavy.lockf (the Mac has no flock; the hub has no named locks).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SSH="$ROOT/build/gpui/mac-ssh"; HOST=hs-mac-mini; DEST=bp-gpui-desktop
COLD=0; CHECK=0; [ "${1:-}" = cold ] && { COLD=1; shift; }; [ "${1:-}" = check ] && { CHECK=1; shift; }
export RSYNC_RSH="$SSH"
$SSH $HOST "mkdir -p ~/$DEST/src/native ~/$DEST/src/assets/web ~/$DEST/dist ~/$DEST/logs"
rsync -a --delete --exclude target --exclude '*.rs.bk' "$ROOT/native/gpui/" $HOST:$DEST/src/native/gpui/
rsync -a "$ROOT/assets/web/" $HOST:$DEST/src/assets/web/
$SSH $HOST "lockf -k /tmp/bp-mac-heavy.lockf sh -s" -- "$COLD" "$CHECK" "$@" <<'REMOTE'
set -eu
COLD=$1; CHECK=$2; shift 2
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer PATH="$HOME/.cargo/bin:$PATH"
cd ~/bp-gpui-desktop/src/native/gpui
export CARGO_TARGET_DIR=$HOME/bp-gpui-desktop/target
[ "$COLD" = 1 ] && rm -rf "$CARGO_TARGET_DIR"
df -h /System/Volumes/Data | tail -1
rustc --version; cargo --version
[ "$CHECK" = 1 ] && { rc=0; cargo check --release "$@" > ~/bp-gpui-desktop/logs/check.log 2>&1 || rc=$?; grep -vE "^\s+(Compiling|Checking)" ~/bp-gpui-desktop/logs/check.log | head -120; exit $rc; }
/usr/bin/time -l cargo build --release "$@" 2> ~/bp-gpui-desktop/logs/build.err || { tail -60 ~/bp-gpui-desktop/logs/build.err; exit 1; }
tail -25 ~/bp-gpui-desktop/logs/build.err
cp "$CARGO_TARGET_DIR/release/backplane-gpui" ~/bp-gpui-desktop/dist/backplane-gpui
ls -l ~/bp-gpui-desktop/dist/backplane-gpui
otool -L ~/bp-gpui-desktop/dist/backplane-gpui
REMOTE
