#!/bin/bash
# Shared by dev-build.sh and cargo-q: how many heavy cargo builds may run at once on this machine, a slot to hold while one runs, and the compiler cache.
#   . scripts/lib-buildslot.sh; bq_acquire [on-wait-callback]; bq_cc; J=$(bq_jobs)
# Slots are flock files in $BUILD_LOCK_DIR (default ~/.cache/backplane-build.locks): a build holds slot.N on fd 9 until its shell exits, the next waits for a free one.
# Env: BUILD_SLOTS (fixed count), BUILD_LOCK_DIR, SCCACHE_DIR (default ~/.cache/sccache), SCCACHE_CACHE_SIZE (default 20G), NO_SCCACHE=1 (no compiler cache).
bq_mem_gb() { awk '/^MemAvailable:/ {print int($2 / 1048576)}' /proc/meminfo 2>/dev/null || echo 16; }
# slots: one per 12 cores and per 6 GB free, between 1 and 3 (this machine: 2)
bq_slots() { [ -n "${BUILD_SLOTS:-}" ] && { echo "$BUILD_SLOTS"; return; }
  local c m n; c=$(( $(nproc) / 12 )); m=$(( $(bq_mem_gb) / 6 )); n=$(( c < m ? c : m )); [ "$n" -lt 1 ] && n=1; [ "$n" -gt 3 ] && n=3; echo "$n"; }
# cargo -j for one build holding a slot: about 2 GB a job from what is free, a share of the cores, at most the 28 the builds are pinned to, at least 1
bq_jobs() { local n g c j; n=$(bq_slots); g=$(( $(bq_mem_gb) / 2 )); c=$(( $(nproc) / n )); j=$g; [ "$c" -lt "$j" ] && j=$c; [ "$j" -gt 28 ] && j=28; [ "$j" -lt 1 ] && j=1; echo "$j"; }
bq_acquire() { local d=${BUILD_LOCK_DIR:-$HOME/.cache/backplane-build.locks} i n waited=0; mkdir -p "$d"
  while :; do n=$(bq_slots); for ((i = 0; i < n; i++)); do exec 9> "$d/slot.$i"; flock -n 9 && return 0; done
    if [ "$waited" = 0 ]; then waited=1; [ -n "${1:-}" ] && "$1"; fi; sleep 1; done; }
# sccache shares compiled dependencies between worktrees (a new lane no longer compiles tokio, naga, zbus... from cold)
bq_cc() { [ -z "${NO_SCCACHE:-}" ] && [ -z "${RUSTC_WRAPPER:-}" ] && command -v sccache > /dev/null 2>&1 || return 0
  export RUSTC_WRAPPER=sccache SCCACHE_DIR=${SCCACHE_DIR:-$HOME/.cache/sccache} SCCACHE_CACHE_SIZE=${SCCACHE_CACHE_SIZE:-20G}; }
