#!/bin/bash
# Shared by dev-build.sh and cargo-q: how many heavy cargo builds may run at once on this machine, a slot to hold while one runs, and the compiler cache.
#   . scripts/lib-buildslot.sh; BQ_HELD=1 bq_run [on-wait-callback] COMMAND...   (inside COMMAND: bq_cc; J=$(bq_jobs))
# Slots are flock files in $BUILD_LOCK_DIR (default ~/.cache/backplane-build.locks): a build holds slot.N while its command runs, the next waits for a free one.
# Env: BUILD_SLOTS (fixed count), BUILD_LOCK_DIR, SCCACHE_DIR (default ~/.cache/sccache), SCCACHE_CACHE_SIZE (default 20G), NO_SCCACHE=1 (no compiler cache).
. "$(dirname "${BASH_SOURCE[0]}")/lib-boat.sh"   # builds on a Boat sandbox when every slot is taken (BQ_OFFLOAD=1 asks for it; BP_NO_BOAT=1 never)
bq_mem_gb() { awk '/^MemAvailable:/ {print int($2 / 1048576)}' /proc/meminfo 2>/dev/null || echo 16; }
# slots: one per 12 cores and per 6 GB free, between 1 and 3 (this machine: 2)
bq_slots() { [ -n "${BUILD_SLOTS:-}" ] && { echo "$BUILD_SLOTS"; return; }
  local c m n; c=$(( $(nproc) / 12 )); m=$(( $(bq_mem_gb) / 6 )); n=$(( c < m ? c : m )); [ "$n" -lt 1 ] && n=1; [ "$n" -gt 3 ] && n=3; echo "$n"; }
# cargo -j for one build holding a slot: about 2 GB a job from what is free, a share of the cores, at most the 28 the builds are pinned to, at least 1
bq_jobs() { local n g c j; n=$(bq_slots); g=$(( $(bq_mem_gb) / 2 )); c=$(( $(nproc) / n )); j=$g; [ "$c" -lt "$j" ] && j=$c; [ "$j" -gt 28 ] && j=28; [ "$j" -lt 1 ] && j=1; echo "$j"; }
# bq_run CALLBACK CMD...: run CMD holding a slot, waiting for a free one (CALLBACK, if any, is run once when it has to wait). `flock -o` closes the lock's descriptor in CMD,
# so nothing CMD leaves running in the background (a sleep, a server) can keep the slot after CMD ends; the flock process itself holds it until CMD exits.
bq_run() { local cb=$1 d=${BUILD_LOCK_DIR:-$HOME/.cache/backplane-build.locks} i n rc waited=0 tick=0; shift; mkdir -p "$d"
  while :; do n=$(bq_slots)
    # the main builder has a slot of its own besides the shared ones: lanes starting builds all day cannot starve it (BQ_MAIN=1, set by main-build.sh; one builder, so one slot)
    if [ -n "${BQ_MAIN:-}" ]; then flock -n -E 200 -o "$d/slot.main" "$@"; rc=$?; [ "$rc" -ne 200 ] && return "$rc"; fi
    for ((i = 0; i < n; i++)); do flock -n -E 200 -o "$d/slot.$i" "$@"; rc=$?; [ "$rc" -ne 200 ] && return "$rc"; done
    # every slot is taken: with BQ_OFFLOAD=1 the same command runs on a Boat box instead of waiting (scripts/lib-boat.sh; the command sees BP_BOAT_IDX and does the run there).
    # 98 = no box could be used (it said why): wait for a local slot as before. 200 = the boxes are busy too: look again every few seconds.
    if [ -n "${BQ_OFFLOAD:-}" ] && [ -z "${BQ_MAIN:-}" ] && [ $((tick % 5)) = 0 ] && bo_ready; then bo_try "$@"; rc=$?; case "$rc" in 98|200) ;; *) return "$rc";; esac; fi
    if [ "$waited" = 0 ]; then waited=1; [ -n "$cb" ] && "$cb"; fi; tick=$((tick + 1)); sleep 1; done; }
# sccache shares compiled dependencies between worktrees (a new lane no longer compiles tokio, naga, zbus... from cold); through sccache-tmp, so the shared server never lives in a test run's TMPDIR
bq_cc() { [ -z "${NO_SCCACHE:-}" ] && [ -z "${RUSTC_WRAPPER:-}" ] && command -v sccache > /dev/null 2>&1 || return 0
  export RUSTC_WRAPPER=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sccache-tmp SCCACHE_DIR=${SCCACHE_DIR:-$HOME/.cache/sccache} SCCACHE_CACHE_SIZE=${SCCACHE_CACHE_SIZE:-20G}; }

# A private temp dir for a test run. The tests make dirs under std::env::temp_dir() (bp-<area>-<pid>-<name>) and never remove them; temp_dir() honours TMPDIR, so every run gets its own
# TMPDIR and drops it when it ends. /tmp here is a RAM tmpfs with an inode quota, and leftovers once stopped every agent shell.
#   t=$(bpt_new); TMPDIR=$t cmd; bpt_end "$t" $?
# The base is ~/.cache/bp-tmp/bpt-XXXXXX (disk, not the /tmp RAM tmpfs; short: unix socket paths are limited to ~108 bytes). A run that failed keeps its dir (marked .bpt-failed) as evidence, and only the BPT_KEEP (default 5)
# newest kept dirs survive; a passing or interrupted run removes its own. Only dirs this function made are ever touched; BPT_BASE (default $BP_TMP, ~/.cache/bp-tmp) moves it.
. "$(dirname "${BASH_SOURCE[0]}")/lib-bptmp.sh"
bpt_new() { mkdir -p "${BPT_BASE:-$BP_TMP}"; mktemp -d "${BPT_BASE:-$BP_TMP}/bpt-XXXXXX"; }
bpt_rm() { [ -n "$1" ] && [ -d "$1" ] && [ ! -L "$1" ] || return 0; chmod -R u+rwX "$1" 2>/dev/null; rm -rf --one-file-system "$1"; }
bpt_end() { local t=$1 rc=${2:-0} k=${BPT_KEEP:-5} d i=0
  [ -n "$t" ] && [ -d "$t" ] || return 0
  if [ "$rc" = 0 ]; then bpt_rm "$t"; return 0; fi
  : > "$t/.bpt-failed"; echo "tests failed (exit $rc): their temp files are kept in $t" >&2
  for d in $(ls -dt "${BPT_BASE:-$BP_TMP}"/bpt-*/ 2>/dev/null); do d=${d%/}; [ -f "$d/.bpt-failed" ] && [ -O "$d" ] && [ ! -L "$d" ] || continue
    i=$((i + 1)); [ "$i" -gt "$k" ] && bpt_rm "$d"; done; return 0; }
