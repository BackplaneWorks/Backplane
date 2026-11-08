#!/bin/sh
# Sync native/gpui to the Mac and build there. Never compiles locally.
# usage: scripts/mac-build.sh [cold|check] [extra cargo args...]
#   MAC_DEST=<dir under ~ on the Mac> (default bp-gpui-desktop): a lane's own copy, so lanes do not build each other's sources
#   GPUI_FEATURES (default cef, as dev-build.sh): with cef, dist/ also gets the embedded Chromium's framework and helper apps (scripts/cef-mac-helpers.sh)
# Takes the Mac-side lock itself: lockf -k /tmp/bp-mac-heavy.lockf (the Mac has no flock; the hub has no named locks).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SSH=${MAC_SSH:-$ROOT/build/gpui/mac-ssh}; [ -x "$SSH" ] || SSH=$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")/build/gpui/mac-ssh
HOST=hs-mac-mini; DEST=${MAC_DEST:-bp-gpui-desktop}; FEAT=${GPUI_FEATURES-cef}
COLD=0; CHECK=0; [ "${1:-}" = cold ] && { COLD=1; shift; }; [ "${1:-}" = check ] && { CHECK=1; shift; }
export RSYNC_RSH="$SSH"
$SSH $HOST "mkdir -p ~/$DEST/src/native/host/src/term ~/$DEST/src/assets/web ~/$DEST/src/scripts ~/$DEST/dist ~/$DEST/logs"
rsync -a --delete --exclude target --exclude '*.rs.bk' "$ROOT/native/gpui/" $HOST:$DEST/src/native/gpui/
rsync -a --exclude cat-rigs "$ROOT/assets/" $HOST:$DEST/src/assets/   # the client compiles in fonts, provider marks and the web colours
rsync -a "$ROOT/native/host/src/term/vt.rs" $HOST:$DEST/src/native/host/src/term/   # the client's terminal parser is the hub's (main.rs `mod vt`)
rsync -a "$ROOT/scripts/cef-mac-helpers.sh" "$ROOT/scripts/cef-runtime-files.sh" $HOST:$DEST/src/scripts/
$SSH $HOST "lockf -k /tmp/bp-mac-heavy.lockf sh -s" -- "$DEST" "$FEAT" "$COLD" "$CHECK" "$@" <<'REMOTE'
set -eu
DEST=$HOME/$1; FEAT=$2; COLD=$3; CHECK=$4; shift 4
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
cd $DEST/src/native/gpui
export CARGO_TARGET_DIR=$DEST/target CEF_PATH=$HOME/.cache/backplane/cef-rs
[ "$COLD" = 1 ] && rm -rf "$CARGO_TARGET_DIR"
df -h /System/Volumes/Data | tail -1
rustc --version; cargo --version
F=; [ -n "$FEAT" ] && F="--features $FEAT"
[ "$CHECK" = 1 ] && { rc=0; cargo check --release $F "$@" > $DEST/logs/check.log 2>&1 || rc=$?; grep -vE "^\s+(Compiling|Checking)" $DEST/logs/check.log | head -120; exit $rc; }
/usr/bin/time -l cargo build --release $F "$@" 2> $DEST/logs/build.err || { tail -60 $DEST/logs/build.err; exit 1; }
tail -25 $DEST/logs/build.err
cp "$CARGO_TARGET_DIR/release/backplane-gpui" $DEST/dist/backplane-gpui
case " $FEAT " in *" cef "*)
  # the framework from the cef crate's distribution (CEF_PATH/<version>/cef_macos_aarch64), the helper apps around backplane-cef-helper
  fw=$(find "$CEF_PATH" -maxdepth 4 -type d -name 'Chromium Embedded Framework.framework' | head -1)
  [ -n "$fw" ] || { echo "no Chromium Embedded Framework.framework under $CEF_PATH"; exit 1; }
  sh $DEST/src/scripts/cef-runtime-files.sh "$(dirname "$fw")" $DEST/dist macos
  sh $DEST/src/scripts/cef-mac-helpers.sh $DEST/dist "$CARGO_TARGET_DIR/release/backplane-cef-helper"
  du -sh $DEST/dist/*.framework $DEST/dist/*.app ;;
esac
ls -l $DEST/dist/backplane-gpui
otool -L $DEST/dist/backplane-gpui
REMOTE
