#!/bin/bash
# Build the hub and window and publish them with dev-cut.sh, keeping the app's progress strip (screens/devbar.rs) up to date.
#   scripts/dev-build.sh [--no-cut] [--gate]    release builds of native/host (backplane-hub), native/voice (backplane-voice) and native/gpui (backplane-gpui), then dev-cut.sh
#   --gate   run the tests of all three crates first (minus scripts/gate-skip.txt); a red test stops the build and nothing is published
# Whatever the flags, a build whose hub cannot start on a real-shaped home is not published (scripts/smoke-start.sh; SKIP_SMOKE=1 only when that script itself is what is broken).
# The strip's bar is time against the last build's time ($DEV/.build-secs, one per mode; a first build guesses 150 s): an incremental build compiles one crate, a full one
# hundreds, so counting crates says nothing. The bar stops at 95% until the build is done.
# When every local build slot is taken this does not wait: it runs the tests and builds on a Boat box (scripts/lib-boat.sh; BP_NO_BOAT=1 forces local) and copies the programs
# back to the paths a local run writes, after a sha256 check; the stage, the smoke start (always, SKIP_SMOKE is ignored for a box's build) and the publish happen here as ever.
# Env: GPUI_DEV_DIR (default ~/.backplane-gpui-dev), BACKPLANE_DEV_STATUS (the status file; default $DEV/BUILD), BUILD_ROOT (the checkout to build; default this one),
#      PROFILE (cargo profile, default fast; release for fat LTO), HOST_TARGET / GPUI_TARGET / VOICE_TARGET (a CARGO_TARGET_DIR for each crate), ASSETS (an assets/ folder to ship; default the one in the build now current), JOBS (default: about 2 GB of free memory a job, a share of the cores, at most 28), BUILD_SLOTS / BUILD_LOCK_DIR / NO_SCCACHE (scripts/lib-buildslot.sh).
set -u
ROOT=${BUILD_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}; HERE=$(cd "$(dirname "$0")" && pwd)
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; ST=${BACKPLANE_DEV_STATUS:-$DEV/BUILD}; J=${JOBS:-}  # cargo -j: JOBS, else sized from free memory once the build lock is held (below)
case " $* " in *" --no-cut "*) [ -n "${BACKPLANE_DEV_STATUS:-}" ] || ST=$DEV/BUILD.nocut.$$;; esac   # a build that publishes nothing must not write the person's progress strip
# A few heavy builds at a time on this machine (scripts/lib-buildslot.sh: slots from cores and free memory, two here): lanes cutting at once queue on a slot instead of all
# compiling together and filling RAM. This script runs again inside the slot (BQ_HELD); nothing it leaves behind can keep the slot.
if [ -z "${BQ_HELD:-}" ]; then . "$HERE/lib-buildslot.sh"; export BQ_HELD=1
  queued() { mkdir -p "$DEV"; printf '{"phase":"building","text":"Queued behind other builds","done":0,"total":0}\n' > "$ST"; }
  bo_ready && export BQ_OFFLOAD=1   # every slot taken: the tests and builds run on a Boat box instead of waiting
  bq_run queued "$0" "$@"; rc=$?; rm -f "$ST"; exit $rc; fi
. "$HERE/lib-buildslot.sh"; bp_notmp_target "${HOST_TARGET:-}" && bp_notmp_target "${GPUI_TARGET:-}" || exit 1   # lane temp and target dirs are on disk (lib-bptmp.sh)
BX=; [ -n "${BP_BOAT_IDX:-}" ] && { BX=$BP_BOAT_IDX; unset BP_BOAT_IDX; }   # started by bq_run under a Boat lease: the cargo steps run on that box (boat_core below)
OFFLINE=--offline; [ -z "${BP_REMOTE_CORE:-}" ] || OFFLINE=   # (a Boat box has the network and no registry copy)
PROFILE=${PROFILE:-fast}  # cargo profile: fast (no LTO, incremental; Cargo.toml) by default, PROFILE=release for the fat-LTO build
GATE=0; CUT=1; for a in "$@"; do case "$a" in --gate) GATE=1;; --no-cut) CUT=0;; esac; done
SECS=$DEV/.build-secs$([ "$GATE" = 1 ] && echo -gate)
HT=${HOST_TARGET:-$ROOT/native/host/target}; GT=${GPUI_TARGET:-$ROOT/native/gpui/target}; VT=${VOICE_TARGET:-$ROOT/native/voice/target}; UT=${GUARD_TARGET:-$HT-guard}
mkdir -p "$DEV"
est=$(cat "$SECS" 2>/dev/null || echo 150); start=$(date +%s); PH=$(mktemp "$DEV/.phase.XXXX"); echo "hub" > "$PH"; LOG=$(mktemp "$DEV/.buildlog.XXXX")
status() { printf '{"phase":"%s","text":"%s","done":%s,"total":%s}\n' "$1" "$2" "${3:-0}" "${4:-0}" > "$ST.$$" && mv -f "$ST.$$" "$ST"; }
# every 2 s: the phase, the seconds, the bar; it is also what keeps the file fresh while one crate compiles for a minute
# the phase: the one being built here, or, on a Boat box, the last one its build said (lines "bp-phase: NAME" in the log)
phase() { if [ -n "$BX" ]; then local p; p=$(tail -c 30000 "$LOG" 2>/dev/null | sed -n 's/^bp-phase: //p' | tail -1); cat "$PH" > /dev/null; echo "${p:-$(cat "$PH" 2>/dev/null)}"; else cat "$PH" 2>/dev/null; fi; }
ticker() { while :; do e=$(( $(date +%s) - start )); d=$e; [ "$d" -gt $((est*95/100)) ] && d=$((est*95/100)); left=$((est-e)); [ "$left" -lt 0 ] && left=0
  [ -n "${BP_REMOTE_CORE:-}" ] && { p=$(cat "$PH" 2>/dev/null); [ "$p" = "${lastp:-}" ] || { echo "bp-phase: $p" >&2; lastp=$p; }; }   # on a Boat box: the phase goes to the caller on stderr
  status building "Building $(phase): ${e}s, about ${left}s to go" "$d" "$est"; sleep 2; done; }
# per-phase seconds, one line each in the build log ("time: compiling tests 41s"), so a slow build says where it went; a phase skipped because its inputs are the last good build's says so ("skip: ...")
TP=$(date +%s.%N); TN=; mark() { local n; n=$(date +%s.%N); [ -z "$TN" ] || printf 'time: %s %.0fs\n' "$TN" "$(awk -v a="$n" -v b="$TP" 'BEGIN{print a-b}')"; TP=$n; TN=$1; [ -z "$1" ] || echo "$1" > "$PH"; }
ticker & TK=$!
# a haiku on what this build adds (commits since MAIN_PREV, the tip built before), made by Haiku while the build runs; a miss costs nothing
HK=$(mktemp "$DEV/.haiku.XXXX"); HKP=
if [ -n "${MAIN_PREV:-}" ]; then "$HERE/build-haiku.sh" "$ROOT" "$MAIN_PREV" "$(git -C "$ROOT" rev-parse HEAD)" > "$HK" 2>/dev/null & HKP=$!; fi
cleanup() { kill $TK $HKP 2>/dev/null; rm -f "$ST" "$ST.$$" "$PH" "$HK"; [ -n "${D:-}" ] && rm -rf "$D"; [ -n "${BOXID:-}" ] && bo_finish "$BX" "$BOXID" "$BJOB"; rm -f "$LOG"
  for t in "${tds[@]}"; do [ -e "$t/.bpt-failed" ] || bpt_rm "$t"; done; }  # test temp dirs (below): a passing, interrupted or killed run leaves none; a failed one's stays
tds=()
trap cleanup EXIT
# The slot is held by the wrapper at the top (below); the compiler cache and the cargo -j (about 2 GB of free memory a job) are set here.
bq_cc; [ -n "$J" ] || J=$(bq_jobs)
mark "hub"
fail() { kill $TK 2>/dev/null; why=$(grep -m1 -E '^(error|test .* FAILED)' "$LOG" | cut -c1-110); printf '%s\n' "$1: ${why:-see the log}" > "$DEV/.last-failure"; cp "$LOG" "$DEV/.last-failure.log" 2>/dev/null
  status failed "Build failed in $1: ${why:-see $DEV/.last-failure.log}" 0 0; tail -20 "$LOG" >&2; [ -n "${BP_REMOTE_CORE:-}" ] || sleep 8; exit 1; }
skips=(); while IFS= read -r l; do case "$l" in ''|'#'*) ;; *) skips+=(--skip "$l");; esac; done < "$HERE/gate-skip.txt"
# cargo runs the tests and the builds one after another; here the tests of both crates are compiled at once (--no-run), then every test program runs at the same time, then the two fast builds
# (a running test program holds no cargo lock). Every test still runs and a red one still stops the build before anything is published.
# the hub says which commit it was compiled from (main.rs commit_of): the guard checks an update against it, so it is set here, not when the build is started
if git -C "$ROOT" rev-parse --short HEAD > /dev/null 2>&1; then export BACKPLANE_COMMIT=$(git -C "$ROOT" rev-parse --short HEAD); fi   # (a Boat box's copy has no .git: the caller's value is passed in)
ncpu=$(nproc); [ "$ncpu" -gt 28 ] && ncpu=28
CG() { ( cd "$1" && CARGO_TARGET_DIR="$2" taskset -c 0-$((ncpu - 1)) nice -n 10 cargo "${@:3}" ) 9>&-; }  # fd 9 is the build slot (lib-buildslot.sh): this shell keeps it, no child may
# The window is built WITH the embedded Chromium (feature cef; docs/browser-gpui-cef.md) unless GPUI_FEATURES is set to something else (empty: none). CEF's distribution is cached once in
# CEF_PATH (the cef crate downloads it on the first build); if that build fails the window is built without it and says so in the Browser tab ("embedded browser unavailable").
FEATURES=${GPUI_FEATURES-cef}; export CEF_PATH=${CEF_PATH:-$HOME/.cache/backplane/cef-rs}
# the voice helper: the hub looks for `backplane-voice` beside itself
voicestep() { mark "voice"; CG "$ROOT/native/voice" "$VT" build --profile "$PROFILE" $OFFLINE -j "$J" --bin backplane-voice > "$LOG" 2>&1 || fail voice; }
winstep() {
  mark "window"
  if [ -n "$FEATURES" ] && ! CG "$ROOT/native/gpui" "$GT" build --profile "$PROFILE" $OFFLINE -j "$J" --features "$FEATURES" > "$LOG" 2>&1; then
    cp "$LOG" "$DEV/.cef-build.log" 2>/dev/null; echo "warning: the window did not build with $FEATURES (see $DEV/.cef-build.log): building it without" >&2; FEATURES=
  fi
  if [ -z "$FEATURES" ]; then CG "$ROOT/native/gpui" "$GT" build --profile "$PROFILE" $OFFLINE -j "$J" > "$LOG" 2>&1 || fail window; fi
}
# `backplane`, the guard (native/guard): a small crate of its own, built first
guardstep() { mark "guard"; CG "$ROOT/native/guard" "$UT" build --profile "$PROFILE" $OFFLINE -j "$J" > "$LOG" 2>&1 || fail guard; }
# On a Boat box (every local slot taken, lib-boat.sh): this same script runs there with BP_REMOTE_CORE=1 (the tests and the four builds, no stage, no publish), its output
# comes back into $LOG, and the programs are copied back to $HT/$GT/$VT/$UT/$PROFILE, hashed on the box and again here. The stage, the smoke start and the publish below stay local.
boat_core() {
  BJOB="$(date +%s)-$$"; mark "boat"; local pullcef=0 ver RT body rc PULL f
  BOXID=$(bo_box "$BX") || { bo_down "no box"; exit 98; }
  bo_say "all build slots are busy: $([ "$GATE" = 1 ] && echo 'testing and ')building on Boat bp-build-$BX ($BOXID)"
  bo_sync "$BOXID" "$ROOT" "$BJOB" || { bo_down "could not copy the worktree to $BOXID"; exit 98; }
  if [ -n "$FEATURES" ]; then ver=$(sed -n 's/^pub const VERSION: &str = "\(.*\)";.*/\1/p' "$ROOT/native/gpui/src/viewers/cef.rs" | head -1); RT=${XDG_CACHE_HOME:-$HOME/.cache}/backplane/cef-runtime/$ver
    { [ -f "$RT/libcef.so" ] && [ -f "$RT/icudtl.dat" ]; } || pullcef=1; fi   # the Chromium runtime is already cached here, or it comes back with the window
  body="export BP_REMOTE_CORE=1 BQ_HELD=1 BP_PULL_CEF=$pullcef PROFILE=$PROFILE BACKPLANE_COMMIT=${BACKPLANE_COMMIT:-} BUILD_ROOT=\$HOME/ws/src GPUI_DEV_DIR=\$BP_JOB/dev CEF_PATH=\$HOME/.cache/backplane/cef-rs"
  body+=" HOST_TARGET=\$HOME/target/host GPUI_TARGET=\$HOME/target/gpui VOICE_TARGET=\$HOME/target/voice GUARD_TARGET=\$HOME/target/guard"$'\n'"exec bash ~/ws/src/scripts/dev-build.sh$([ "$GATE" = 1 ] && echo ' --gate') --no-cut"
  bo_run "$BOXID" "$BJOB" "" "$body" > "$LOG" 2>&1; rc=$?
  [ "$rc" = 98 ] && { bo_down "the build could not run on $BOXID"; exit 98; }
  [ "$rc" = 0 ] || fail "$(phase)"
  grep '^gate: ' "$LOG"
  f=$(sed -n 's/^bp-features: //p' "$LOG" 2>/dev/null | tail -1); [ -z "$f" ] || FEATURES=$f   # (what the box ended up building the window with)
  mark "copying"; PULL=$(mktemp -d "$DEV/.pull.XXXX")
  bo_pull "$BOXID" "$BJOB" /home/user/target "$PULL" >> "$LOG" 2>&1 || { rm -rf "$PULL"; fail "copy from the box"; }
  cpart() { mkdir -p "$2/$PROFILE"; cp -f "$PULL/$1/$PROFILE/$3" "$2/$PROFILE/"; }
  cpart host "$HT" backplane-hub; cpart gpui "$GT" backplane-gpui; cpart voice "$VT" backplane-voice; cpart guard "$UT" backplane
  [ "$pullcef" = 1 ] && cp -a "$PULL/gpui/$PROFILE/." "$GT/$PROFILE/"
  rm -rf "$PULL"; echo "built on Boat bp-build-$BX ($BOXID)" > "$DEV/.built-on-boat"; }
if [ -n "$BX" ]; then boat_core
elif [ "$GATE" = 1 ]; then
  mark "guard tests"; CG "$ROOT/native/guard" "$UT" test $OFFLINE -j "$J" > "$LOG" 2>&1 || fail "guard tests"
  mark "compiling tests"; LH=$(mktemp "$DEV/.t.XXXX"); LW=$(mktemp "$DEV/.t.XXXX"); LV=$(mktemp "$DEV/.t.XXXX")
  CG "$ROOT/native/host" "$HT" test $OFFLINE -j "$J" --no-run > "$LH" 2>&1 & ph=$!
  CG "$ROOT/native/gpui" "$GT" test $OFFLINE -j "$J" --no-run > "$LW" 2>&1 & pw=$!
  # the voice tests take env!("CARGO_BIN_EXE_backplane-voice"), which came out undefined through sccache ("not defined at compile time") and is fine without it; its deps are already built in $VT
  RUSTC_WRAPPER= CG "$ROOT/native/voice" "$VT" test $OFFLINE -j "$J" --no-run > "$LV" 2>&1 & pv=$!
  wait $ph || { cp "$LH" "$LOG"; rm -f "$LH" "$LW" "$LV"; fail "hub tests"; }
  wait $pw || { cp "$LW" "$LOG"; rm -f "$LH" "$LW" "$LV"; fail "window tests"; }
  wait $pv || { cp "$LV" "$LOG"; rm -f "$LH" "$LW" "$LV"; fail "voice tests"; }
  mark "testing"; names=(); pids=(); logs=(); st=()
  # each test program gets its own TMPDIR (tests make dirs under temp_dir() and never remove them; lib-buildslot.sh bpt_new), removed after a pass, kept (bounded) after a failure
  run() { n=$1; dir=$2; shift 2; l=$(mktemp "$DEV/.t.XXXX"); t=$(bpt_new); tds+=("$t"); ( cd "$dir" && TMPDIR=$t CARGO_MANIFEST_DIR="$dir" "$@" ) > "$l" 2>&1 9>&- & pids+=($!); names+=("$n"); logs+=("$l")
    # a machine with fewer than 16 cores (a Boat box) runs the test programs one after another: side by side they starve each other's timing-sensitive tests (a recorder "that fails at once" is judged within 500 ms)
    if [ "$ncpu" -lt 16 ]; then wait "${pids[-1]}"; st[${#pids[@]}-1]=$?; fi; }
  for x in $(sed -n 's/^ *Executable .*(\(.*\))$/\1/p' "$LH"); do run "hub tests" "$ROOT/native/host" "$x" "${skips[@]}"; done
  for x in $(sed -n 's/^ *Executable .*(\(.*\))$/\1/p' "$LW"); do run "window tests" "$ROOT/native/gpui" "$x"; done
  for x in $(sed -n 's/^ *Executable .*(\(.*\))$/\1/p' "$LV"); do run "voice tests" "$ROOT/native/voice" "$x"; done
  rm -f "$LH" "$LW" "$LV"
  bad=; for i in "${!pids[@]}"; do { if [ -n "${st[$i]+x}" ]; then [ "${st[$i]}" = 0 ]; else wait "${pids[$i]}"; fi; } && bpt_end "${tds[$i]}" 0 || { [ -n "$bad" ] || bad=$i; bpt_end "${tds[$i]}" 1; }; done
  if [ -n "$bad" ]; then cp "${logs[$bad]}" "$LOG"; rm -f "${logs[@]}"; fail "${names[$bad]}"; fi
  # what ran, per suite (so two runs, here and on a Boat box, can be compared): "gate: hub tests 1234 passed, 0 failed, 3 ignored"
  for n in "hub tests" "window tests" "voice tests"; do for i in "${!names[@]}"; do [ "${names[$i]}" = "$n" ] && cat "${logs[$i]}"; done | awk -v n="$n" '/^test result:/ {p+=$4; f+=$6; g+=$8} END {printf "gate: %s %d passed, %d failed, %d ignored\n", n, p, f, g}'; done
  rm -f "${logs[@]}"
  # the builds come after the tests, not beside them: a build at -j28 starves the timing-sensitive hub tests (reconnect_loses_nothing_under_streaming_load and others fail under that load)
  guardstep
  mark "hub"; CG "$ROOT/native/host" "$HT" build --profile "$PROFILE" $OFFLINE -j "$J" --bin backplane-hub > "$LOG" 2>&1 || fail hub
  voicestep
  winstep
else
  guardstep
  mark "hub";    CG "$ROOT/native/host" "$HT" build --profile "$PROFILE" $OFFLINE -j "$J" --bin backplane-hub > "$LOG" 2>&1 || fail hub
  voicestep
  winstep
fi
if [ -n "${BP_REMOTE_CORE:-}" ]; then   # on a Boat box: the programs and their hashes, for the caller to copy back (lib-boat.sh bo_pull); no stage here
  kill $TK 2>/dev/null; echo "bp-features: $FEATURES" >&2; T=$HOME/target; cd "$T" || exit 1
  fl=("${HT#$T/}/$PROFILE/backplane-hub" "${GT#$T/}/$PROFILE/backplane-gpui" "${VT#$T/}/$PROFILE/backplane-voice" "${UT#$T/}/$PROFILE/backplane")
  if [ "${BP_PULL_CEF:-0}" = 1 ] && [ -f "$GT/$PROFILE/libcef.so" ]; then for f in libcef.so resources.pak chrome_100_percent.pak chrome_200_percent.pak icudtl.dat v8_context_snapshot.bin; do fl+=("${GT#$T/}/$PROFILE/$f"); done
    for f in "$GT/$PROFILE/locales/"*.pak; do fl+=("${f#$T/}"); done; fi
  printf '%s\n' "${fl[@]}" > "$BP_JOB/artifacts"; sha256sum "${fl[@]}" > "$BP_JOB/sha256"; exit 0
fi
mark ""; kill $TK 2>/dev/null; echo $(( $(date +%s) - start )) > "$SECS"; rm -f "$DEV/.last-failure" "$DEV/.last-failure.log"
D=$(mktemp -d "$DEV/.stage.XXXX"); mark "stage"
cp "$HT/$PROFILE/backplane-hub" "$GT/$PROFILE/backplane-gpui" "$VT/$PROFILE/backplane-voice" "$UT/$PROFILE/backplane" "$D/"
cp "$ROOT/native/voice/vad/silero_vad.onnx" "$D/"   # dictation's VAD beside the helper, as in a release: local-install takes it (no download) when it matches its pin
# the embedded Chromium: libcef.so beside the window (it is linked with RUNPATH $ORIGIN); the rest of CEF's runtime (paks, icudtl.dat, locales, snapshot) is found in
# ~/.cache/backplane/cef-runtime/<version>, which is made once here from the build (libcef stripped: 1.4 GB with symbols, ~270 MB without), and builds link to that one libcef.
ver=$(sed -n 's/^pub const VERSION: &str = "\(.*\)";.*/\1/p' "$ROOT/native/gpui/src/viewers/cef.rs" | head -1); RT=${XDG_CACHE_HOME:-$HOME/.cache}/backplane/cef-runtime/$ver
if [ -n "$FEATURES" ] && { [ -f "$GT/$PROFILE/libcef.so" ] || [ -f "$RT/libcef.so" ]; }; then
  if [ ! -f "$RT/libcef.so" ] || [ ! -f "$RT/icudtl.dat" ]; then
    mkdir -p "$RT/locales"; for f in resources.pak chrome_100_percent.pak chrome_200_percent.pak icudtl.dat v8_context_snapshot.bin; do [ -f "$RT/$f" ] || cp "$GT/$PROFILE/$f" "$RT/"; done
    cp -n "$GT/$PROFILE/locales/"*.pak "$RT/locales/" 2>/dev/null; cp "$GT/$PROFILE/libcef.so" "$RT/libcef.so.tmp" && strip --strip-unneeded "$RT/libcef.so.tmp" 2>/dev/null; mv "$RT/libcef.so.tmp" "$RT/libcef.so"
  fi
  ln -s "$RT/libcef.so" "$D/libcef.so"
fi
mark "browser helper"
# the browser helper (tools/browser, bun): the hub looks for `backplane-browser` beside itself, and without one every Browser tab fails to open
command -v bun >/dev/null 2>&1 && { . "$ROOT/scripts/bun-pin.sh"; bun_pin_check bun; }   # a release builds it with deploy/bun-version's Bun
if command -v bun >/dev/null 2>&1 && ( cd "$ROOT/tools/browser" && bun install --frozen-lockfile >/dev/null 2>&1 && bun build.ts "$D/backplane-browser" >/dev/null 2>&1 ); then :
else prev=$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/backplane-browser; [ -x "$prev" ] && cp "$prev" "$D/" || echo "warning: no browser helper built (bun missing or the build failed) and none to reuse" >&2; fi
mark "assets"
A=${ASSETS:-$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)/assets}; [ -d "$A" ] && cp -r "$A" "$D/assets"
git -C "$ROOT" rev-parse --short HEAD > "$D/COMMIT"
[ -z "$HKP" ] || wait "$HKP" 2>/dev/null; [ -s "$HK" ] && cp "$HK" "$D/HAIKU"
# the last gate: this hub, under this guard, starts and restarts on a real-shaped home, and the build now current can still read what it wrote. Nothing is published otherwise.
if [ -z "${SKIP_SMOKE:-}" ] || [ -n "$BX" ]; then   # (a build made on a Boat box is always smoke-started here, whatever SKIP_SMOKE says)
  mark "smoke start"; status building "Smoke start of $(cat "$D/COMMIT")" "$est" "$est"
  SMOKE_DIR="$DEV" "$HERE/smoke-start.sh" "$D" "$DEV/builds/$(head -1 "$DEV/CURRENT" 2>/dev/null)" > "$LOG" 2>&1 || fail "smoke start"
fi
[ "$CUT" = 0 ] && { mark ""; echo "time: total $(( $(date +%s) - start ))s"; echo "built in $D"; D=; exit 0; }
status ready "Build $(cat "$D/COMMIT") ready: publishing" "$est" "$est"; mark "publish"
"$HERE/dev-cut.sh" "$D" 9>&-; rc=$?; mark ""; echo "time: total $(( $(date +%s) - start ))s"; exit $rc
