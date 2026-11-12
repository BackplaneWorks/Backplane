#!/bin/bash
# Shared by dev-build.sh and cargo-q: how many heavy cargo builds may run at once on this machine, a slot to hold while one runs, and the compiler cache.
#   . scripts/lib-buildslot.sh; BQ_HELD=1 bq_run [on-wait-callback] COMMAND...   (inside COMMAND: bq_cc; J=$(bq_jobs))
# Slots are flock files in $BUILD_LOCK_DIR (default ~/.cache/backplane-build.locks): a build holds slot.N while its command runs, the next waits for a free one.
# Env: BUILD_SLOTS (fixed count), BUILD_LOCK_DIR, SCCACHE_DIR (default ~/.cache/sccache), SCCACHE_CACHE_SIZE (default 20G), NO_SCCACHE=1 (no compiler cache).
. "$(dirname "${BASH_SOURCE[0]}")/lib-boat.sh"   # builds on a Boat sandbox when every slot is taken (BQ_OFFLOAD=1 asks for it; BP_NO_BOAT=1 never)
bq_mem_gb() { awk '/^MemAvailable:/ {print int($2 / 1048576)}' /proc/meminfo 2>/dev/null || echo 16; }
# slots: one per 10 cores and per 6 GB free, between 1 and 4 (this desktop, 32 cores and ~42 GB free: 3). The person's rule (Nov 11): this machine is used fully before a Boat.
bq_slots() { [ -n "${BUILD_SLOTS:-}" ] && { echo "$BUILD_SLOTS"; return; }
  local c m n; c=$(( $(nproc) / 10 )); m=$(( $(bq_mem_gb) / 6 )); n=$(( c < m ? c : m )); [ "$n" -lt 1 ] && n=1; [ "$n" -gt 4 ] && n=4; echo "$n"; }
# a build goes to a Boat only when every local slot is taken AND (it has already waited BQ_OFFLOAD_AFTER seconds, default 420, or free memory is under BQ_LOW_GB, default 8)
bq_offload_now() { [ "$1" -ge "${BQ_OFFLOAD_AFTER:-420}" ] || [ "$(bq_mem_gb)" -lt "${BQ_LOW_GB:-8}" ]; }
# cargo -j for one build holding a slot: about 2 GB a job from this slot's share of what is free (every slot may fill at once: each sizing itself from all the free memory
# is how three builds asked for 60 GB on a 62 GB machine, Nov 12), a share of the cores, at most the 28 the builds are pinned to, at least 1
bq_jobs() { local n g c j; n=$(bq_slots); g=$(( $(bq_mem_gb) / (2 * n) )); c=$(( $(nproc) / n )); j=$g; [ "$c" -lt "$j" ] && j=$c; [ "$j" -gt 28 ] && j=28; [ "$j" -lt 1 ] && j=1; echo "$j"; }
# BQ_SCOPE: a build runs in a transient scope of its own in bpbuild.slice, never in the scope of whoever started it. An agent's build used to run inside the hub's scope
# (the hub, its agents, their builds and tests: 28 GB), so systemd-oomd's pressure kill took the whole hub with it (Nov 12 01:25 and 02:16). Now a build is its own
# candidate: oomd kills the build, the hub's scope (ManagedOOMPreference=avoid, fresh.sh) is picked last. MemoryMax (BQ_MEM_MAX, 60%) stops one runaway build alone.
# No user systemd (a Boat box, the Mac, a container) or BQ_NO_SCOPE=1: the command runs as it is.
bq_scope() { BQ_SCOPE=()
  [ -z "${BQ_NO_SCOPE:-}" ] && command -v systemd-run > /dev/null 2>&1 && [ -S "${XDG_RUNTIME_DIR:-/nonexistent}/bus" ] || return 0
  BQ_SCOPE=(systemd-run --user --scope -q --collect --slice=bpbuild.slice -p "MemoryMax=${BQ_MEM_MAX:-60%}" -p ManagedOOMPreference=none --); }
# bq_run CALLBACK CMD...: run CMD holding a slot, waiting for a free one (CALLBACK, if any, is run once when it has to wait, with the number of local builds ahead of it). `flock -o` closes the lock's descriptor in CMD,
# so nothing CMD leaves running in the background (a sleep, a server) can keep the slot after CMD ends; the flock process itself holds it until CMD exits.
bq_run() { local cb=$1 d=${BUILD_LOCK_DIR:-$HOME/.cache/backplane-build.locks} i n rc waited=0 tick=0; shift; mkdir -p "$d"; bq_scope
  while :; do n=$(bq_slots)
    # the main builder has a slot of its own besides the shared ones: lanes starting builds all day cannot starve it (BQ_MAIN=1, set by main-build.sh; one builder, so one slot)
    if [ -n "${BQ_MAIN:-}" ]; then flock -n -E 200 -o "$d/slot.main" ${BQ_SCOPE[@]+"${BQ_SCOPE[@]}"} "$@"; rc=$?; [ "$rc" -ne 200 ] && return "$rc"; fi
    for ((i = 0; i < n; i++)); do flock -n -E 200 -o "$d/slot.$i" ${BQ_SCOPE[@]+"${BQ_SCOPE[@]}"} "$@"; rc=$?; [ "$rc" -ne 200 ] && return "$rc"; done
    # every slot is taken: with BQ_OFFLOAD=1, and only once the wait is long or memory is low (bq_offload_now), the same command runs on a Boat box (scripts/lib-boat.sh;
    # the command sees BP_BOAT_IDX and does the run there). 98 = no box could be used (it said why): keep waiting locally. 200 = the boxes are busy too: look again every few seconds.
    if [ -n "${BQ_OFFLOAD:-}" ] && [ -z "${BQ_MAIN:-}" ] && [ $((tick % 5)) = 0 ] && bq_offload_now "$tick" && bo_ready; then bo_try "$@"; rc=$?; case "$rc" in 98|200) ;; *) return "$rc";; esac; fi
    if [ "$waited" = 0 ]; then waited=1; [ -n "$cb" ] && "$cb" "$n"; fi; tick=$((tick + 1)); sleep 1; done; }
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
