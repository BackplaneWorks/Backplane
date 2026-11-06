#!/bin/sh
# Ship the committed tree (git archive of HEAD; the Mac has only openrsync) (native, assets, docs, test, i18n) to the Mac and build the hub and the client there. Never compiles locally.
# usage: scripts/mac-hub-build.sh [hub|gpui|both] [test | test:FILTER]   (default both; test:FILTER runs the matching tests only and prints their output)   -> ~/bp-e2e/dist/{backplane-hub,backplane-gpui}
# Takes the Mac-side lock itself: lockf -k /tmp/bp-mac-heavy.lockf (the Mac has no flock; the hub has no named locks). Needs ~/.cargo or /opt/homebrew/bin cargo on the Mac.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WHAT=${1:-both}; TEST=${2:-}
HOST=${MAC_HOST:-hs-mac-mini}; DEST=bp-e2e
SSH="ssh -o BatchMode=yes"; [ -f "$ROOT/build/gpui/mac-ssh" ] && SSH="$ROOT/build/gpui/mac-ssh"
$SSH $HOST "mkdir -p ~/$DEST/src ~/$DEST/dist ~/$DEST/logs"
git -C "$ROOT" archive --format=tar HEAD native assets docs test i18n | $SSH $HOST "rm -rf ~/$DEST/src && mkdir -p ~/$DEST/src && tar -x -C ~/$DEST/src"
$SSH $HOST "lockf -k /tmp/bp-mac-heavy.lockf sh -s" -- "$WHAT" "$TEST" <<'REMOTE'
set -eu
WHAT=${1:-both}; TEST=${2:-}
while ! mkdir /tmp/bp-mac-heavy.lock 2>/dev/null; do sleep 20; done; trap 'rmdir /tmp/bp-mac-heavy.lock' EXIT
ulimit -n 10240 2>/dev/null || true
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
export CARGO_TARGET_DIR=$HOME/bp-e2e/target CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6}
df -h /System/Volumes/Data | tail -1; rustc --version
build() { # crate dir, binary
  cd ~/bp-e2e/src/native/$1
  case "$TEST" in test:*) F=${TEST#test:}; cargo test --release --bin backplane-hub "$F" -- --nocapture --test-threads=1 2>&1 | grep -v "^warning\|^ *|\|^ *=\|^ *-->\|^$" | tail -60; return 0;; esac
  if [ -n "$TEST" ]; then cargo test --release --no-fail-fast > ~/bp-e2e/logs/test-$1.log 2>&1 || echo "tests failed: $1 (logs/test-$1.log)"; grep -E "^test result|FAILED|failed" ~/bp-e2e/logs/test-$1.log | tail -15; fi
  cargo build --release > ~/bp-e2e/logs/build-$1.log 2>&1 || { tail -60 ~/bp-e2e/logs/build-$1.log; exit 1; }
  cp "$CARGO_TARGET_DIR/release/$2" ~/bp-e2e/dist/; ls -l ~/bp-e2e/dist/$2
}
# separate target dirs: the two crates are not in one workspace
case $WHAT in hub|both) CARGO_TARGET_DIR=$HOME/bp-e2e/target-hub build host backplane-hub;; esac
case $WHAT in gpui|both) CARGO_TARGET_DIR=$HOME/bp-e2e/target-gpui build gpui backplane-gpui;; esac
REMOTE
