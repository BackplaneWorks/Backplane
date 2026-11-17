#!/bin/bash
# Lane temp space lives on disk, never in /tmp (a 30 GB RAM tmpfs with a quota: lanes filled it twice and stopped every agent shell).
#   . scripts/lib-bptmp.sh      -> BP_TMP (default ~/.cache/bp-tmp) exists; TMPDIR and CLAUDE_CODE_TMPDIR point at it unless already set elsewhere than /tmp
# Short on purpose: unix socket paths are limited to ~108 bytes.
# BP_DATA: where heavy scratch lives: /data when this machine has it (a 2.2 TB build and scratch drive since Nov 17), else ~/.cache. BP_TARGET, sccache,
# main-build's targets and new worktrees' target/ dirs (bp_target_link) are under it. BP_TMP is not: it is TMPDIR, and an AppImage (FreeCAD) mounts itself under
# TMPDIR with fusermount3, whose AppArmor profile allows only $HOME, /tmp, /mnt, /media and /run/user (Nov 17: TMPDIR on /data failed every FreeCAD test with
# "fusermount3: mount failed: Permission denied"). It stays on the home disk, held under 60 GB by the janitor.
if [ -z "${BP_DATA:-}" ]; then if [ -d /data ] && [ -w /data ]; then BP_DATA=/data; else BP_DATA=$HOME/.cache; fi; fi; export BP_DATA
BP_TMP=${BP_TMP:-$HOME/.cache/bp-tmp}; mkdir -p "$BP_TMP"; export BP_TMP
case "${TMPDIR:-/tmp}" in /tmp|/tmp/*) export TMPDIR=$BP_TMP;; esac
case "${CLAUDE_CODE_TMPDIR:-/tmp}" in /tmp|/tmp/*) export CLAUDE_CODE_TMPDIR=$BP_TMP/claude;; esac
mkdir -p "$CLAUDE_CODE_TMPDIR" 2>/dev/null
# BP_TARGET (default ~/.cache/bp-target): shared cargo target dirs for builds that are not a worktree's own (cross builds: $BP_TARGET/<target>), one per target for
# every lane instead of one-off $BP_TMP/target-* folders (Nov 17: three of those were 30 GB); the janitor keeps it under GC_TARGET_CAP (40G), oldest unused first,
# and the compiled dependencies come from sccache either way.
BP_TARGET=${BP_TARGET:-$BP_DATA/bp-target}; export BP_TARGET
# bp_target_link CRATE_DIR [NAME]: a crate's target dir (NAME, default target) that does not exist yet is made a link to $BP_DATA/target/<checkout>-<hash>/<crate>[-NAME],
# so a worktree's builds go to the data drive while every path (native/host/target/fast/backplane-hub) stays the same. Nothing is done without a data drive, or
# when the dir exists (a real dir, or a link already). The janitor follows these links (idle, landed) and removes what a gone worktree left (scripts/worktree-gc.sh).
bp_target_link() { local c=$1 n=${2:-target} top id; [ "$BP_DATA" != "$HOME/.cache" ] || return 0; [ -e "$c/$n" ] || [ -L "$c/$n" ] && return 0
  top=$(git -C "$c" rev-parse --show-toplevel 2>/dev/null) || return 0
  id=$(basename "$top")-$(printf '%s' "$top" | sha1sum | cut -c1-8)
  mkdir -p "$BP_DATA/target/$id/$(basename "$c")${2:+-$2}" && ln -sfn "$BP_DATA/target/$id/$(basename "$c")${2:+-$2}" "$c/$n"; }
# bp_notmp_target DIR: refuse a cargo target dir under /tmp (it is RAM; one is 4 GB)
bp_notmp_target() { case "${1:-}" in /tmp|/tmp/*) echo "refusing CARGO_TARGET_DIR=$1: /tmp is RAM. Use a dir on disk (the worktree's own target/, or \$BP_TARGET/<target> for a cross build)." >&2; return 1;; esac; }
