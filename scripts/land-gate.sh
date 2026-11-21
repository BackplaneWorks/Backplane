#!/bin/bash
# The merge gate scripts/land.sh runs (its default LAND_TEST): fast, on a Boat box, for the crates the PR touches only.
# A crate is touched when the PR (origin/main...HEAD) changes a file under it, or a file outside it that its build or tests read
# (scripts/hub-outside-files.py --crate: #[path], include_*!, CARGO_MANIFEST_DIR joins, build.rs inputs, path dependencies).
# For each touched crate of native/{host,gpui,guard,voice}, through scripts/cargo-q on a Boat box (BUILD_SLOTS=0, BQ_OFFLOAD_AFTER=0):
#   1. cargo check --all-targets --locked
#   2. cargo test --locked: its unit tests, skipping every test whose name contains "e2e" (and, for the hub, scripts/gate-skip.txt)
# Not here (the release-candidate pass, docs/release-playbook.md): Windows and macOS runs, the e2e suites, the screenshot and size matrix,
# the KiCad embed suites. A PR touching no crate (docs, scripts, mobile, web sources) compiles nothing and passes.
# Says what ran and how long each step took. Exit 0 passed, 1 a step failed.
# Env: LAND_GATE_CARGO (the cargo command; default scripts/cargo-q), LAND_GATE_BASE (origin/main), LAND_GATE_LOCAL=1 (no Boat).
set -u
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
cargo=${LAND_GATE_CARGO:-$ROOT/scripts/cargo-q}
# every local slot "taken": cargo-q goes to a Boat box at once. BP_NO_BOAT=1 (a lead's stopgap: build here) or LAND_GATE_LOCAL=1: the local slots,
# as for any build. Either way a wait for a slot ends after LAND_GATE_WAIT_MAX (300) seconds with a line, never holding the land lock forever.
[ -n "${LAND_GATE_LOCAL:-}${BP_NO_BOAT:-}" ] || export BUILD_SLOTS=0 BQ_OFFLOAD_AFTER=0
export BQ_WAIT_MAX=${LAND_GATE_WAIT_MAX:-300}
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
  touched "$d" || continue
  step "$c: cargo check --all-targets" "$d" check --all-targets --locked --offline || { rc=1; continue; }
  if [ "$c" = host ]; then step "$c: unit tests" "$d" test --locked --offline -- --skip e2e "${skips[@]}" || rc=1
  elif [ "$c" = voice ]; then RUSTC_WRAPPER= step "$c: unit tests" "$d" test --locked --offline -- --skip e2e || rc=1
  else step "$c: unit tests" "$d" test --locked --offline -- --skip e2e || rc=1; fi
done
dt=$(( $(date +%s) - t0 ))
if [ -z "$ran" ]; then say "nothing to compile: the PR touches no crate (docs, scripts, mobile, web sources) ($(git diff --name-only "$base" HEAD | wc -l) files); passed in ${dt}s"
elif [ "$rc" = 0 ]; then say "passed in $((dt / 60))m$((dt % 60))s: $ran"
else say "FAILED after $((dt / 60))m$((dt % 60))s: $ran"; fi
exit "$rc"
