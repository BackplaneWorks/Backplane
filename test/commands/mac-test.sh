#!/bin/sh
# From the repo root, under the mac-heavy lock (work_run lock:"mac-heavy"): cargo test of the commands module on the Mac (Boat is the first choice; this is for when its daily start limit is used up).
# Fixtures and stand-in agents only. usage: sh test/commands/mac-test.sh [FILTER]
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); SSH="$ROOT/../../../build/gpui/mac-ssh"; [ -x "$SSH" ] || SSH=$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")/build/gpui/mac-ssh
HOST=hs-mac-mini; DEST=bp-gpui-commands; F=${1:-commands::}
export RSYNC_RSH="$SSH"
$SSH $HOST "mkdir -p ~/$DEST/src/core ~/$DEST/target"
rsync -a --delete --exclude target --exclude '*.rs.bk' "$ROOT/native/host/" $HOST:$DEST/native/host/
rsync -a --delete --exclude '*.tgz' --exclude '*.gz' "$ROOT/test/" $HOST:$DEST/test/
rsync -a "$ROOT/src/core/cbor" "$ROOT/src/core/codec" $HOST:$DEST/src/core/
$SSH $HOST "sh -s" -- "$F" <<'REMOTE'
set -u
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
df -h /System/Volumes/Data | tail -1
cd ~/bp-gpui-commands/native/host
CARGO_TARGET_DIR=$HOME/bp-gpui-commands/target cargo test --bin backplane-hub "$1" -- --test-threads=2 > $HOME/bp-gpui-commands/test.log 2>&1; rc=$?; grep -E "^error|-->|test result|FAILED|panicked|left:|right:" $HOME/bp-gpui-commands/test.log | head -60; exit $rc
REMOTE
