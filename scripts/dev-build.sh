#!/bin/bash
# Build the hub and window and publish them with dev-cut.sh, keeping the app's progress strip (screens/devbar.rs) up to date.
#   scripts/dev-build.sh [--no-cut] [--gate]    release builds of native/host (backplane-hub) and native/gpui (backplane-gpui), then dev-cut.sh
#   --gate   run the tests of both crates first (minus scripts/gate-skip.txt); a red test stops the build and nothing is published
# The strip's bar is time against the last build's time ($DEV/.build-secs, one per mode; a first build guesses 150 s): an incremental build compiles one crate, a full one
# hundreds, so counting crates says nothing. The bar stops at 95% until the build is done.
# Env: GPUI_DEV_DIR (default ~/.backplane-gpui-dev), BACKPLANE_DEV_STATUS (the status file; default $DEV/BUILD), BUILD_ROOT (the checkout to build; default this one),
#      PROFILE (cargo profile, default fast; release for fat LTO), HOST_TARGET / GPUI_TARGET (a CARGO_TARGET_DIR for each crate), ASSETS (an assets/ folder to ship; default the one in the build now current), JOBS (default: about 2 GB of free memory a job, a share of the cores, at most 28), BUILD_SLOTS / BUILD_LOCK_DIR / NO_SCCACHE (scripts/lib-buildslot.sh).
set -u
ROOT=${BUILD_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}; HERE=$(cd "$(dirname "$0")" && pwd)
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; ST=${BACKPLANE_DEV_STATUS:-$DEV/BUILD}; J=${JOBS:-}  # cargo -j: JOBS, else sized from free memory once the build lock is held (below)
# A few heavy builds at a time on this machine (scripts/lib-buildslot.sh: slots from cores and free memory, two here): lanes cutting at once queue on a slot instead of all
# compiling together and filling RAM. This script runs again inside the slot (BQ_HELD); nothing it leaves behind can keep the slot.
if [ -z "${BQ_HELD:-}" ]; then . "$HERE/lib-buildslot.sh"; export BQ_HELD=1
  queued() { mkdir -p "$DEV"; printf '{"phase":"building","text":"Queued behind other builds","done":0,"total":0}\n' > "$ST"; }
  bq_run queued "$0" "$@"; rc=$?; exit $rc; fi
. "$HERE/lib-buildslot.sh"
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
# a haiku on what this build adds (commits since MAIN_PREV, the tip built before), made by Haiku while the build runs; a miss costs nothing
HK=$(mktemp "$DEV/.haiku.XXXX"); HKP=
if [ -n "${MAIN_PREV:-}" ]; then "$HERE/build-haiku.sh" "$ROOT" "$MAIN_PREV" "$(git -C "$ROOT" rev-parse HEAD)" > "$HK" 2>/dev/null & HKP=$!; fi
cleanup() { kill $TK $HKP 2>/dev/null; rm -f "$ST" "$ST.$$" "$PH" "$HK"; [ -n "${D:-}" ] && rm -rf "$D"; rm -f "$LOG"
  for t in "${tds[@]}"; do [ -e "$t/.bpt-failed" ] || bpt_rm "$t"; done; }  # test temp dirs (below): a passing, interrupted or killed run leaves none; a failed one's stays
tds=()
trap cleanup EXIT
# The slot is held by the wrapper at the top (below); the compiler cache and the cargo -j (about 2 GB of free memory a job) are set here.
bq_cc; [ -n "$J" ] || J=$(bq_jobs)
echo "hub" > "$PH"
fail() { kill $TK 2>/dev/null; why=$(grep -m1 -E '^(error|test .* FAILED)' "$LOG" | cut -c1-110); printf '%s\n' "$1: ${why:-see the log}" > "$DEV/.last-failure"; cp "$LOG" "$DEV/.last-failure.log" 2>/dev/null
  status failed "Build failed in $1: ${why:-see $DEV/.last-failure.log}" 0 0; tail -20 "$LOG" >&2; sleep 8; exit 1; }
skips=(); while IFS= read -r l; do case "$l" in ''|'#'*) ;; *) skips+=(--skip "$l");; esac; done < "$HERE/gate-skip.txt"
# cargo runs the tests and the builds one after another; here the tests of both crates are compiled at once (--no-run), then every test program runs at the same time, then the two fast builds
# (a running test program holds no cargo lock). Every test still runs and a red one still stops the build before anything is published.
CG() { ( cd "$1" && CARGO_TARGET_DIR="$2" taskset -c 0-27 nice -n 10 cargo "${@:3}" ) 9>&-; }  # fd 9 is the build slot (lib-buildslot.sh): this shell keeps it, no child may
# The window is built WITH the embedded Chromium (feature cef; docs/browser-gpui-cef.md) unless GPUI_FEATURES is set to something else (empty: none). CEF's distribution is cached once in
# CEF_PATH (the cef crate downloads it on the first build); if that build fails the window is built without it and says so in the Browser tab ("embedded browser unavailable").
FEATURES=${GPUI_FEATURES-cef}; export CEF_PATH=${CEF_PATH:-$HOME/.cache/backplane/cef-rs}
winstep() {
  echo "window" > "$PH"
  if [ -n "$FEATURES" ] && ! CG "$ROOT/native/gpui" "$GT" build --profile "$PROFILE" --offline -j "$J" --features "$FEATURES" > "$LOG" 2>&1; then
    cp "$LOG" "$DEV/.cef-build.log" 2>/dev/null; echo "warning: the window did not build with $FEATURES (see $DEV/.cef-build.log): building it without" >&2; FEATURES=
  fi
  if [ -z "$FEATURES" ]; then CG "$ROOT/native/gpui" "$GT" build --profile "$PROFILE" --offline -j "$J" > "$LOG" 2>&1 || fail window; fi
}
if [ "$GATE" = 1 ]; then
  echo "compiling tests" > "$PH"; LH=$(mktemp "$DEV/.t.XXXX"); LW=$(mktemp "$DEV/.t.XXXX")
  CG "$ROOT/native/host" "$HT" test --offline -j "$J" --no-run > "$LH" 2>&1 & ph=$!
  CG "$ROOT/native/gpui" "$GT" test --offline -j "$J" --no-run > "$LW" 2>&1 & pw=$!
  wait $ph || { cp "$LH" "$LOG"; rm -f "$LH" "$LW"; fail "hub tests"; }
  wait $pw || { cp "$LW" "$LOG"; rm -f "$LH" "$LW"; fail "window tests"; }
  echo "testing" > "$PH"; names=(); pids=(); logs=()
  # each test program gets its own TMPDIR (tests make dirs under temp_dir() and never remove them; lib-buildslot.sh bpt_new), removed after a pass, kept (bounded) after a failure
  run() { n=$1; dir=$2; shift 2; l=$(mktemp "$DEV/.t.XXXX"); t=$(bpt_new); tds+=("$t"); ( cd "$dir" && TMPDIR=$t CARGO_MANIFEST_DIR="$dir" "$@" ) > "$l" 2>&1 9>&- & pids+=($!); names+=("$n"); logs+=("$l"); }
  for x in $(sed -n 's/^ *Executable .*(\(.*\))$/\1/p' "$LH"); do run "hub tests" "$ROOT/native/host" "$x" "${skips[@]}"; done
  for x in $(sed -n 's/^ *Executable .*(\(.*\))$/\1/p' "$LW"); do run "window tests" "$ROOT/native/gpui" "$x"; done
  rm -f "$LH" "$LW"
  bad=; for i in "${!pids[@]}"; do wait "${pids[$i]}" && bpt_end "${tds[$i]}" 0 || { [ -n "$bad" ] || bad=$i; bpt_end "${tds[$i]}" 1; }; done
  if [ -n "$bad" ]; then cp "${logs[$bad]}" "$LOG"; rm -f "${logs[@]}"; fail "${names[$bad]}"; fi
  rm -f "${logs[@]}"
  # the builds come after the tests, not beside them: a build at -j28 starves the timing-sensitive hub tests (reconnect_loses_nothing_under_streaming_load and others fail under that load)
  echo "hub" > "$PH"; CG "$ROOT/native/host" "$HT" build --profile "$PROFILE" --offline -j "$J" --bin backplane-hub > "$LOG" 2>&1 || fail hub
  winstep
else
  echo "hub" > "$PH";    CG "$ROOT/native/host" "$HT" build --profile "$PROFILE" --offline -j "$J" --bin backplane-hub > "$LOG" 2>&1 || fail hub
  winstep
fi
kill $TK 2>/dev/null; echo $(( $(date +%s) - start )) > "$SECS"; rm -f "$DEV/.last-failure" "$DEV/.last-failure.log"
D=$(mktemp -d "$DEV/.stage.XXXX")
cp "$HT/$PROFILE/backplane-hub" "$GT/$PROFILE/backplane-gpui" "$D/"
# the embedded Chromium: libcef.so beside the window (it is linked with RUNPATH $ORIGIN); the rest of CEF's runtime (paks, icudtl.dat, locales, snapshot) is found in
# ~/.cache/backplane/cef-runtime/<version>, which is made once here from the build (libcef stripped: 1.4 GB with symbols, ~270 MB without), and builds link to that one libcef.
if [ -n "$FEATURES" ] && [ -f "$GT/$PROFILE/libcef.so" ]; then
  ver=$(sed -n 's/^pub const VERSION: &str = "\(.*\)";.*/\1/p' "$ROOT/native/gpui/src/viewers/cef.rs" | head -1); RT=${XDG_CACHE_HOME:-$HOME/.cache}/backplane/cef-runtime/$ver
  if [ ! -f "$RT/libcef.so" ] || [ ! -f "$RT/icudtl.dat" ]; then
    mkdir -p "$RT/locales"; for f in resources.pak chrome_100_percent.pak chrome_200_percent.pak icudtl.dat v8_context_snapshot.bin; do [ -f "$RT/$f" ] || cp "$GT/$PROFILE/$f" "$RT/"; done
    cp -n "$GT/$PROFILE/locales/"*.pak "$RT/locales/" 2>/dev/null; cp "$GT/$PROFILE/libcef.so" "$RT/libcef.so.tmp" && strip --strip-unneeded "$RT/libcef.so.tmp" 2>/dev/null; mv "$RT/libcef.so.tmp" "$RT/libcef.so"
  fi
  ln -s "$RT/libcef.so" "$D/libcef.so"
fi
# the browser helper (tools/browser, bun): the hub looks for `backplane-browser` beside itself, and without one every Browser tab fails to open
if command -v bun >/dev/null 2>&1 && ( cd "$ROOT/tools/browser" && bun install --frozen-lockfile >/dev/null 2>&1 && bun build.ts "$D/backplane-browser" >/dev/null 2>&1 ); then :
else prev=$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/backplane-browser; [ -x "$prev" ] && cp "$prev" "$D/" || echo "warning: no browser helper built (bun missing or the build failed) and none to reuse" >&2; fi
A=${ASSETS:-$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/assets}; [ -d "$A" ] && cp -r "$A" "$D/assets"
git -C "$ROOT" rev-parse --short HEAD > "$D/COMMIT"
[ -z "$HKP" ] || wait "$HKP" 2>/dev/null; [ -s "$HK" ] && cp "$HK" "$D/HAIKU"
[ "$CUT" = 0 ] && { echo "built in $D"; D=; exit 0; }
status ready "Build $(cat "$D/COMMIT") ready: publishing" "$est" "$est"
"$HERE/dev-cut.sh" "$D" 9>&-
