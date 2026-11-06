#!/bin/bash
# worktree-gc.sh [--apply] [--days N] [--hours N] [--quiet]: free the disk lanes' worktrees take.
#   - a worktree under .backplane/worktrees whose branch is merged into main, whose tree is clean (no untracked files either), that nothing has used for N days (default 1)
#     and that no process works in is removed with its target/ dirs (the branch is deleted too: it is merged);
#   - the target/ dirs of any other worktree that no build has written to for --hours (default 6) are deleted (a rebuild fills them again, with sccache mostly from the cache).
# Dry run unless --apply. Never touches the checkout that has main out, a detached worktree, or one with unmerged commits or edits.
set -u
APPLY=0; DAYS=1; HOURS=6; QUIET=0; while [ $# -gt 0 ]; do case "$1" in --apply) APPLY=1;; --days) DAYS=$2; shift;; --hours) HOURS=$2; shift;; --quiet) QUIET=1;; esac; shift; done
REPO=$(git -C "$(dirname "$0")" worktree list --porcelain | awk 'NR==1{print substr($0,10)}'); BR=${MAIN_BRANCH:-main}
say() { [ "$QUIET" = 1 ] || echo "$@"; }
CWDS=$(mktemp); trap 'rm -f "$CWDS"' EXIT; for c in /proc/[0-9]*/cwd; do readlink "$c" 2>/dev/null; done | sort -u > "$CWDS"
busy() { grep -qxF -e "$1" "$CWDS" || grep -qF -e "$1/" "$CWDS"; }
idle() { [ -z "$(find "$1" -maxdepth 1 -mtime -"$2" -print -quit 2>/dev/null)" ] && [ -z "$(find "$1/native" -maxdepth 2 \( -name target -prune \) -o -mtime -"$2" -print -quit 2>/dev/null)" ] \
  && [ -z "$(find "$(git -C "$1" rev-parse --git-dir)" -maxdepth 1 \( -name index -o -name HEAD \) -mtime -"$2" -print -quit 2>/dev/null)" ]; }
git -C "$REPO" worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch /{print w "\t" substr($0,8)}' | while IFS=$'\t' read -r w ref; do
  case "$w" in */.backplane/worktrees/*) ;; *) continue;; esac
  [ -d "$w" ] || continue; busy "$w" && continue
  b=${ref#refs/heads/}
  if git -C "$REPO" merge-base --is-ancestor "$ref" "$BR" 2>/dev/null && [ -z "$(git -C "$w" status --porcelain 2>/dev/null)" ] && idle "$w" "$DAYS"; then
    say "remove  $w ($b merged, clean, idle)"
    [ "$APPLY" = 1 ] && git -C "$REPO" worktree remove --force "$w" 2>/dev/null && git -C "$REPO" branch -d "$b" > /dev/null 2>&1
    continue
  fi
  for t in "$w"/native/*/target; do [ -d "$t" ] || continue; [ -z "$(find "$t" -maxdepth 2 -mmin -$(( HOURS * 60 )) -print -quit 2>/dev/null)" ] || continue
    say "target  $(( $(du -sk "$t" | cut -f1) / 1024 )) MB  $t"; [ "$APPLY" = 1 ] && rm -rf "$t"; done
done
