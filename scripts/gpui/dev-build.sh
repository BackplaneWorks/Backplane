#!/bin/bash
# Build the hub and window from this worktree and publish them with dev-cut.sh, keeping the app's progress strip (screens/devbar.rs) up to date.
#   scripts/gpui/dev-build.sh [--no-cut]     release builds of native/host (backplane-hub) and native/gpui (backplane-gpui), then dev-cut.sh
# The strip's bar is time against the last build's time ($DEV/.build-secs; a first build guesses 150 s): an incremental build compiles one crate, a full one hundreds,
# so counting crates says nothing. The bar stops at 95% until the build is done.
# Env: GPUI_DEV_DIR (default ~/.backplane-gpui-dev), BACKPLANE_DEV_STATUS (the status file), ASSETS (an assets/ folder to ship; default the one in the build now current).
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; ST=${BACKPLANE_DEV_STATUS:-$DEV/STATUS}; SECS=$DEV/.build-secs
mkdir -p "$DEV"
est=$(cat "$SECS" 2>/dev/null || echo 150); start=$(date +%s); PH=$(mktemp "$DEV/.phase.XXXX"); echo "hub" > "$PH"; LOG=$(mktemp "$DEV/.buildlog.XXXX")
status() { printf '{"phase":"%s","text":"%s","done":%s,"total":%s}\n' "$1" "$2" "${3:-0}" "${4:-0}" > "$ST.$$" && mv -f "$ST.$$" "$ST"; }
# every 2 s: the phase, the seconds, the bar; it is also what keeps the file fresh while one crate compiles for a minute
ticker() { while :; do e=$(( $(date +%s) - start )); d=$e; [ "$d" -gt $((est*95/100)) ] && d=$((est*95/100)); left=$((est-e)); [ "$left" -lt 0 ] && left=0
  status building "Building $(cat "$PH" 2>/dev/null): ${e}s, about ${left}s to go" "$d" "$est"; sleep 2; done; }
ticker & TK=$!
cleanup() { kill $TK 2>/dev/null; rm -f "$ST" "$ST.$$" "$PH" "$LOG"; [ -n "${D:-}" ] && rm -rf "$D"; }
trap cleanup EXIT
fail() { kill $TK 2>/dev/null; status failed "Build failed in $1: $(grep -m1 '^error' "$LOG" | cut -c1-120)" 0 0; cat "$LOG" | tail -20 >&2; sleep 8; exit 1; }
echo hub > "$PH";    ( cd "$ROOT/native/host" && taskset -c 0-27 nice -n 10 cargo build --release --offline --bin backplane-hub ) > "$LOG" 2>&1 || fail hub
echo window > "$PH"; ( cd "$ROOT/native/gpui" && taskset -c 0-27 nice -n 10 cargo build --release --offline ) > "$LOG" 2>&1 || fail window
kill $TK 2>/dev/null; echo $(( $(date +%s) - start )) > "$SECS"
D=$(mktemp -d "$DEV/.stage.XXXX")
cp "$ROOT/native/host/target/release/backplane-hub" "$ROOT/native/gpui/target/release/backplane-gpui" "$D/"
A=${ASSETS:-$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/assets}; [ -d "$A" ] && cp -r "$A" "$D/assets"
git -C "$ROOT" rev-parse --short HEAD > "$D/COMMIT"
[ "${1:-}" = "--no-cut" ] && { echo "built in $D"; D=; exit 0; }
status ready "Build $(cat "$D/COMMIT") ready: publishing" "$est" "$est"
"$ROOT/scripts/gpui/dev-cut.sh" "$D"
