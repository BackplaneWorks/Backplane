#!/bin/bash
# The merge gate scripts/land.sh runs (its default LAND_TEST): fast, on a Boat box, for the crates the PR touches only.
# A crate is touched when the PR (origin/main...HEAD) changes a file under it, or a file outside it that its build or tests read
# (scripts/hub-outside-files.py --crate: #[path], include_*!, CARGO_MANIFEST_DIR joins, build.rs inputs, path dependencies).
# For each touched crate of native/{host,gpui,guard,voice}, through scripts/cargo-q on a Boat box (BUILD_SLOTS=0, BQ_OFFLOAD_AFTER=0):
#   1. cargo check --all-targets --locked
#   2. cargo test --locked: its unit tests, skipping every test whose name contains "e2e" (and, for the hub, scripts/gate-skip.txt)
#   3. host and gpui: the Windows cross-compile check (scripts/windows-check.sh: cargo check --target x86_64-pc-windows-gnu --all-targets, with
#      llvm-mingw; on the Boat box, which fetches it once): a Windows-only compile error (cfg(windows) code, a Windows trait clash) fails here
# Not here (the release-candidate pass, docs/release-playbook.md): Windows and macOS runs, the e2e suites, the screenshot and size matrix,
# the KiCad embed suites. A PR touching no crate (docs, scripts, mobile, web sources) compiles nothing and passes.
# Says what ran and how long each step took. Exit 0 passed, 1 a step failed.
# Env: LAND_GATE_CARGO (the cargo command; default scripts/cargo-q), LAND_GATE_WINCHECK (default bash scripts/windows-check.sh), LAND_GATE_BASE
# (origin/main), LAND_GATE_LOCAL=1 (no Boat).
set -u
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
cargo=${LAND_GATE_CARGO:-$ROOT/scripts/cargo-q}
# every local slot "taken": cargo-q goes to a Boat box at once, from a pool of LAND_GATE_BOATS (8) boxes (each stops itself after 5 idle minutes:
# boat-pool.sh reap). BP_NO_BOAT=1 (a lead's stopgap: build here) or LAND_GATE_LOCAL=1: the local slots, as for any build. The gate holds no lock
# (land.sh gates outside it), so a gate that finds no slot and no box waits its turn in a fair queue (BQ_FAIR: first come, first served, its place
# said) for up to LAND_GATE_WAIT_MAX (2700 s, 45 min), then says so.
[ -n "${LAND_GATE_LOCAL:-}${BP_NO_BOAT:-}" ] || export BUILD_SLOTS=0 BQ_OFFLOAD_AFTER=0 BP_BOAT_POOL=${LAND_GATE_BOATS:-8}
export BQ_WAIT_MAX=${LAND_GATE_WAIT_MAX:-2700} BQ_FAIR=1
base=$(git merge-base "${LAND_GATE_BASE:-origin/main}" HEAD) || { echo "gate: no merge base with ${LAND_GATE_BASE:-origin/main}" >&2; exit 1; }
changed=$(git diff --name-only "$base" HEAD)
say() { echo "gate: $*" >&2; }
t0=$(date +%s); ran=; rc=0
touched() { # touched CRATE_DIR: a changed file is under it, or among what it reads outside it
  printf '%s\n' "$changed" | grep -q "^$1/" && return 0
  out=$(python3 -I "$ROOT/scripts/hub-outside-files.py" --crate "$1" "$ROOT") || { say "could not list what $1 reads outside it: checking it"; return 0; }
  for f in $out; do printf '%s\n' "$changed" | grep -q -x -e "$f" -e "$f/.*" && return 0; done; return 1; }
step() { # step NAME DIR ARGS...: one cargo command, timed
  local name=$1 d=$2; shift 2; local s=$(date +%s)
  say "$name ..."
  if (cd "$d" && "$cargo" "$@"); then say "$name: ok ($(( $(date +%s) - s ))s)"; ran="${ran:+$ran, }$name $(( $(date +%s) - s ))s"; return 0; fi
  say "$name: FAILED ($(( $(date +%s) - s ))s)"; ran="${ran:+$ran, }$name FAILED"; return 1; }
skips=(); [ -f "$ROOT/scripts/gate-skip.txt" ] && while IFS= read -r l; do case "$l" in ''|'#'*) ;; *) skips+=(--skip "$l");; esac; done < "$ROOT/scripts/gate-skip.txt"
for c in host gpui guard voice; do
  d=native/$c; [ -d "$d" ] || continue
  # what of the PR this crate builds from: a file under it, or one it reads outside it
  # what it reads outside it; not known (the scanner failed): every change counts, and the crate is built
  if out=$(python3 -I "$ROOT/scripts/hub-outside-files.py" --crate "$d" "$ROOT"); then scan=1; else say "$c: could not list what it reads outside $d: building it"; out=; scan=; fi
  if [ -z "$scan" ]; then hits=$(printf '%s\n' "$changed" | grep .); else
  hits=$(printf '%s\n' "$changed" | while IFS= read -r f; do [ -n "$f" ] || continue
    case "$f" in "$d"/*) echo "$f"; continue;; esac
    for i in $out; do if [ "$f" = "$i" ] || [ "${f#"$i"/}" != "$f" ]; then echo "$f"; break; fi; done; done); fi
  [ -n "$hits" ] || continue
  # web/dist alone (the hub embeds it, it does not compile it): no cargo, no build slot; a file the crate's sources name must not go
  if [ -n "$scan" ] && [ -z "$(printf '%s\n' "$hits" | grep -v '^web/dist/')" ]; then
    gone=; while IFS=$'\t' read -r st old _; do [ -n "$old" ] || continue; git grep -q -F "$(basename "$old")" HEAD -- "$d/src" && gone="$gone $old"; done \
      < <(git -c core.quotepath=off diff --name-status --diff-filter=DR "$base" HEAD -- web/dist)
    if [ -n "$gone" ]; then say "$c: it embeds a file the PR deletes or renames:$gone"; ran="${ran:+$ran, }$c embeds FAILED"; rc=1
    else say "$c: web/dist only: every file it names is there; no build"; ran="${ran:+$ran, }$c embeds ok"; fi
    continue
  fi
  step "$c: cargo check --all-targets" "$d" check --all-targets --locked --offline || { rc=1; continue; }
  if [ "$c" = host ]; then step "$c: unit tests" "$d" test --locked --offline -- --skip e2e "${skips[@]}" || rc=1
  elif [ "$c" = voice ]; then RUSTC_WRAPPER= step "$c: unit tests" "$d" test --locked --offline -- --skip e2e || rc=1
  else step "$c: unit tests" "$d" test --locked --offline -- --skip e2e || rc=1; fi
  case "$c" in host|gpui) s=$(date +%s); say "$c: Windows cross-compile check ..."
    if ${LAND_GATE_WINCHECK:-bash "$ROOT/scripts/windows-check.sh"} "$c"; then say "$c: Windows cross-compile check: ok ($(( $(date +%s) - s ))s)"; ran="${ran:+$ran, }$c: Windows check $(( $(date +%s) - s ))s"
    else say "$c: Windows cross-compile check: FAILED ($(( $(date +%s) - s ))s)"; ran="${ran:+$ran, }$c: Windows check FAILED"; rc=1; fi;; esac
done
dt=$(( $(date +%s) - t0 ))
if [ -z "$ran" ]; then say "nothing to compile: the PR touches no crate (docs, scripts, mobile, web sources) ($(git diff --name-only "$base" HEAD | wc -l) files); passed in ${dt}s"
elif [ "$rc" = 0 ]; then say "passed in $((dt / 60))m$((dt % 60))s: $ran"
else say "FAILED after $((dt / 60))m$((dt % 60))s: $ran"; fi
exit "$rc"
