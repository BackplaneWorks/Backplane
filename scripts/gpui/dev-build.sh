#!/bin/bash
# Build the hub and window from this worktree and publish them with dev-cut.sh, keeping the app's progress strip (screens/devbar.rs) up to date.
#   scripts/gpui/dev-build.sh [--no-cut]     release builds of native/host (backplane-hub) and native/gpui (backplane-gpui), then dev-cut.sh
# The strip's numbers: cargo's own "Compiling" lines counted against the last build's count ($DEV/.build-units; a first build guesses 600).
# Env: GPUI_DEV_DIR (default ~/.backplane-gpui-dev), BACKPLANE_DEV_STATUS (the status file), ASSETS (an assets/ folder to ship; default the one in the build now current).
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; ST=${BACKPLANE_DEV_STATUS:-$DEV/STATUS}; UNITS=$DEV/.build-units
mkdir -p "$DEV"
status() { printf '{"phase":"%s","text":"%s","done":%s,"total":%s}\n' "$1" "$2" "${3:-0}" "${4:-0}" > "$ST.$$" && mv -f "$ST.$$" "$ST"; }
done_() { rm -f "$ST" "$ST.$$"; }
trap 'done_' EXIT
total=$(cat "$UNITS" 2>/dev/null || echo 600); n=0
hb() { while :; do sleep 4; [ -f "$ST" ] && touch "$ST"; done; }   # a long single crate prints nothing: keep the strip alive
status building "Building..." 0 "$total"
hb & HB=$!; trap 'kill $HB 2>/dev/null; done_' EXIT
fail() { kill $HB 2>/dev/null; status failed "Build failed: $1 (see the build output)" 0 0; sleep 6; exit 1; }
run() {  # run DIR NAME LABEL : cargo with a live count of crates
  local fifo; fifo=$(mktemp -u); mkfifo "$fifo"
  ( cd "$ROOT/native/$1" && taskset -c 0-5 nice -n 19 cargo build --release --offline ${2:+--bin $2} > "$fifo" 2>&1; echo "rc=$?" > "$fifo.rc" ) &
  while IFS= read -r line < "$fifo"; do case "$line" in *Compiling*) n=$((n+1)); status building "Building $3: crate $n of ~$total" "$n" "$total";; esac; done
  wait; rm -f "$fifo"; local rc; rc=$(sed 's/rc=//' "$fifo.rc"); rm -f "$fifo.rc"; return "${rc:-1}"
}
run host backplane-hub hub || fail hub
run gpui "" window || fail window
echo "$n" > "$UNITS"
D=$(mktemp -d "$DEV/.stage.XXXX"); trap 'rm -rf "$D"; kill $HB 2>/dev/null; done_' EXIT
cp "$ROOT/native/host/target/release/backplane-hub" "$ROOT/native/gpui/target/release/backplane-gpui" "$D/"
A=${ASSETS:-$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/assets}; [ -d "$A" ] && cp -r "$A" "$D/assets"
git -C "$ROOT" rev-parse --short HEAD > "$D/COMMIT"
[ "${1:-}" = "--no-cut" ] && { echo "built in $D"; trap - EXIT; kill $HB 2>/dev/null; exit 0; }
status ready "Build $(cat "$D/COMMIT") ready: publishing" "$total" "$total"
"$ROOT/scripts/gpui/dev-cut.sh" "$D"
