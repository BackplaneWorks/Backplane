#!/bin/bash
# Lane temp space lives on disk, never in /tmp (a 30 GB RAM tmpfs with a quota: lanes filled it twice and stopped every agent shell).
#   . scripts/lib-bptmp.sh      -> BP_TMP (default ~/.cache/bp-tmp) exists; TMPDIR and CLAUDE_CODE_TMPDIR point at it unless already set elsewhere than /tmp
# Short on purpose: unix socket paths are limited to ~108 bytes.
BP_TMP=${BP_TMP:-$HOME/.cache/bp-tmp}; mkdir -p "$BP_TMP"; export BP_TMP
case "${TMPDIR:-/tmp}" in /tmp|/tmp/*) export TMPDIR=$BP_TMP;; esac
case "${CLAUDE_CODE_TMPDIR:-/tmp}" in /tmp|/tmp/*) export CLAUDE_CODE_TMPDIR=$BP_TMP/claude;; esac
mkdir -p "$CLAUDE_CODE_TMPDIR" 2>/dev/null
# bp_notmp_target DIR: refuse a cargo target dir under /tmp (it is RAM; one is 4 GB)
bp_notmp_target() { case "${1:-}" in /tmp|/tmp/*) echo "refusing CARGO_TARGET_DIR=$1: /tmp is RAM. Use a dir on disk (e.g. \$BP_TMP/target-<lane>, or the worktree's own target/)." >&2; return 1;; esac; }
