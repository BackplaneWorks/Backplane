#!/bin/bash
# Build the hub and window and publish them with dev-cut.sh, keeping the app's progress strip (screens/devbar.rs) up to date.
#   scripts/dev-build.sh [--no-cut] [--gate]    release builds of native/host (backplane-hub) and native/gpui (backplane-gpui), then dev-cut.sh
#   --gate   run the tests of both crates first (minus scripts/gate-skip.txt); a red test stops the build and nothing is published
# The strip's bar is time against the last build's time ($DEV/.build-secs, one per mode; a first build guesses 150 s): an incremental build compiles one crate, a full one
# hundreds, so counting crates says nothing. The bar stops at 95% until the build is done.
# Env: GPUI_DEV_DIR (default ~/.backplane-gpui-dev), BACKPLANE_DEV_STATUS (the status file; default $DEV/BUILD), BUILD_ROOT (the checkout to build; default this one),
#      PROFILE (cargo profile, default fast; release for fat LTO), HOST_TARGET / GPUI_TARGET (a CARGO_TARGET_DIR for each crate), ASSETS (an assets/ folder to ship; default the one in the build now current), JOBS (default 28).
set -u
ROOT=${BUILD_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}; HERE=$(cd "$(dirname "$0")" && pwd)
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; ST=${BACKPLANE_DEV_STATUS:-$DEV/BUILD}; J=${JOBS:-28}
PROFILE=${PROFILE:-fast}  # cargo profile: fast (no LTO, incremental; Cargo.toml) by default, PROFILE=release for the fat-LTO build
GATE=0; CUT=1; for a in "$@"; do case "$a" in --gate) GATE=1;; --no-cut) CUT=0;; esac; done
SECS=$DEV/.build-secs$([ "$GATE" = 1 ] && echo -gate)
HT=${HOST_TARGET:-$ROOT/native/host/target}; GT=${GPUI_TARGET:-$ROOT/native/gpui/target}
mkdir -p "$DEV"
est=$(cat "$SECS" 2>/dev/null || echo 150); start=$(date +%s); PH=$(mktemp "$DEV/.phase.XXXX"); echo "hub" > "$PH"; LOG=$(mktemp "$DEV/.buildlog.XXXX")
status() { printf '{"phase":"%s","text":"%s","done":%s,"total":%s}\n' "$1" "$2" "${3:-0}" "${4:-0}" > "$ST.$$" && mv -f "$ST.$$" "$ST"; }
# every 2 s: the phase, the seconds, the bar; it is also what keeps the file fresh while one crate compiles for a minute
ticker() { while :; do e=$(( $(date +%s) - start )); d=$e; [ "$d" -gt $((est*95/100)) ] && d=$((est*95/100)); left=$((est-e)); [ "$left" -lt 0 ] && left=0
  status building "Building $(cat "$PH" 2>/dev/null): ${e}s, about ${left}s to go" "$d" "$est"; sleep 2; done; }
ticker & TK=$!
cleanup() { kill $TK 2>/dev/null; rm -f "$ST" "$ST.$$" "$PH"; [ -n "${D:-}" ] && rm -rf "$D"; rm -f "$LOG"; }
trap cleanup EXIT
fail() { kill $TK 2>/dev/null; why=$(grep -m1 -E '^(error|test .* FAILED)' "$LOG" | cut -c1-110); printf '%s\n' "$1: ${why:-see the log}" > "$DEV/.last-failure"; cp "$LOG" "$DEV/.last-failure.log" 2>/dev/null
  status failed "Build failed in $1: ${why:-see $DEV/.last-failure.log}" 0 0; tail -20 "$LOG" >&2; sleep 8; exit 1; }
skips=(); while IFS= read -r l; do case "$l" in ''|'#'*) ;; *) skips+=(--skip "$l");; esac; done < "$HERE/gate-skip.txt"
if [ "$GATE" = 1 ]; then
  echo "testing hub" > "$PH";    ( cd "$ROOT/native/host" && CARGO_TARGET_DIR="$HT" taskset -c 0-27 nice -n 10 cargo test --offline -j "$J" -- "${skips[@]}" ) > "$LOG" 2>&1 || fail "hub tests"
  echo "testing window" > "$PH"; ( cd "$ROOT/native/gpui" && CARGO_TARGET_DIR="$GT" taskset -c 0-27 nice -n 10 cargo test --offline -j "$J" ) > "$LOG" 2>&1 || fail "window tests"
fi
echo "hub" > "$PH";    ( cd "$ROOT/native/host" && CARGO_TARGET_DIR="$HT" taskset -c 0-27 nice -n 10 cargo build --profile "$PROFILE" --offline -j "$J" --bin backplane-hub ) > "$LOG" 2>&1 || fail hub
echo "window" > "$PH"; ( cd "$ROOT/native/gpui" && CARGO_TARGET_DIR="$GT" taskset -c 0-27 nice -n 10 cargo build --profile "$PROFILE" --offline -j "$J" ) > "$LOG" 2>&1 || fail window
kill $TK 2>/dev/null; echo $(( $(date +%s) - start )) > "$SECS"; rm -f "$DEV/.last-failure" "$DEV/.last-failure.log"
D=$(mktemp -d "$DEV/.stage.XXXX")
cp "$HT/$PROFILE/backplane-hub" "$GT/$PROFILE/backplane-gpui" "$D/"
# the browser helper (tools/browser, bun): the hub looks for `backplane-browser` beside itself, and without one every Browser tab fails to open
if command -v bun >/dev/null 2>&1 && ( cd "$ROOT/tools/browser" && bun install --frozen-lockfile >/dev/null 2>&1 && bun build.ts "$D/backplane-browser" >/dev/null 2>&1 ); then :
else prev=$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/backplane-browser; [ -x "$prev" ] && cp "$prev" "$D/" || echo "warning: no browser helper built (bun missing or the build failed) and none to reuse" >&2; fi
A=${ASSETS:-$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/assets}; [ -d "$A" ] && cp -r "$A" "$D/assets"
git -C "$ROOT" rev-parse --short HEAD > "$D/COMMIT"
[ "$CUT" = 0 ] && { echo "built in $D"; D=; exit 0; }
status ready "Build $(cat "$D/COMMIT") ready: publishing" "$est" "$est"
"$HERE/dev-cut.sh" "$D"
