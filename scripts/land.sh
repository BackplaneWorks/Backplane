#!/bin/sh
# Lands the branch checked out here: upstream main (origin/main) is the one main; local main only ever follows it.
# usage: scripts/land.sh                 run from your lane's worktree, on your branch, with everything committed
#   1. git fetch origin main;
#   2. git rebase origin/main (a conflict stops here: the rebase is aborted and nothing moved; rebase by hand, then land again);
#   3. checks origin/main..HEAD (scripts/check-author.sh);
#   4. free space: the build and the tests' scratch (the repo's disk and $TMPDIR/~/.cache's) each need LAND_MIN_FREE_GB (10);
#      less fails at once ("land: FAILED: only N GB free on /, need M: clean up") instead of as unrelated test failures;
#      tests: scripts/dev-build.sh --gate --no-cut (LAND_TEST overrides the command; it is skipped only when this exact tree
#      passed it already, recorded in <git-dir>/land-tested);
#   5. git push origin HEAD:main, a fast-forward (never forced). Rejected because origin moved: back to 1, up to LAND_TRIES (3);
#   6. fast-forwards local main to what was pushed (never ahead of origin; the main hook refuses that anyway).
# Any step that cannot be done fails loudly with "land: FAILED ..." and a non-zero exit; it never leaves local main ahead of origin.
set -eu
cd "$(git rev-parse --show-toplevel)"
here=$(cd "$(dirname "$0")" && pwd)
die() { echo "land: FAILED: $*" >&2; exit 1; }
say() { echo "land: $*" >&2; }
# A full disk fails a build or a dozen unrelated tests ("No space left on device"); say so first, plainly
need_gb=${LAND_MIN_FREE_GB:-10}
free_check() {
  for d in . "${TMPDIR:-/tmp}" "$HOME/.cache"; do
    [ -d "$d" ] || continue
    set -- $(df -Pk "$d" 2>/dev/null | awk 'NR == 2 { print $4, $6 }')
    [ -n "${1:-}" ] || continue
    gb=$(( $1 / 1048576 ))
    [ "$gb" -ge "$need_gb" ] || die "only $gb GB free on $2, need $need_gb: clean up (old cargo targets, test scratch in ~/.cache/bp-tmp)"
  done
}
br=$(git symbolic-ref --short -q HEAD) || die "HEAD is detached; land from your lane's branch"
[ "$br" != main ] || die "you are on main; commit on your lane's branch (from origin/main) and land that"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "uncommitted changes in $(pwd); commit them first"
git remote get-url origin > /dev/null 2>&1 || die "no origin remote"
mark=$(git rev-parse --git-dir)/land-tested
tries=${LAND_TRIES:-3}; n=0
while :; do
  n=$((n + 1))
  git fetch -q origin main || die "git fetch origin main failed (offline?); nothing pushed"
  if ! git rebase -q origin/main; then git rebase --abort 2>/dev/null || true; die "git rebase origin/main conflicts; resolve it by hand (git rebase origin/main), then land again"; fi
  [ -n "$(git rev-list -1 origin/main..HEAD)" ] || { say "nothing to land: $br is already on origin/main"; break; }
  "$here/check-author.sh" origin/main..HEAD || die "a commit has the wrong author or committer (see above)"
  free_check
  tree=$(git rev-parse 'HEAD^{tree}')
  if [ "$(cat "$mark" 2>/dev/null)" = "$tree" ]; then say "tests already passed on this tree ($(printf %s "$tree" | cut -c1-10))"
  else
    say "testing $(git rev-parse --short HEAD) on origin/main $(git rev-parse --short origin/main)"
    sh -c "${LAND_TEST:-scripts/dev-build.sh --gate --no-cut}" || die "tests failed; nothing pushed"
    printf '%s\n' "$tree" > "$mark"
  fi
  if git push origin "HEAD:refs/heads/main"; then break; fi
  git fetch -q origin main || die "push rejected and fetch failed; nothing pushed"
  if git merge-base --is-ancestor HEAD origin/main; then break; fi   # (it went in after all)
  [ "$n" -lt "$tries" ] || die "push rejected $n times (origin/main keeps moving, or the remote refuses: see above); nothing pushed"
  say "origin/main moved to $(git rev-parse --short origin/main): fetching, rebasing and testing again ($((n + 1))/$tries)"
done
tip=$(git rev-parse HEAD)
say "origin/main is $(git rev-parse --short "$tip")"
# local main follows: fast-forward it in the checkout that has it (the main hook then starts the main builder)
mainwt=$(git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch refs\/heads\/main$/{print w}')
if git merge-base --is-ancestor main "$tip" 2>/dev/null; then
  if [ -n "$mainwt" ]; then git -C "$mainwt" merge -q --ff-only "$tip" || die "pushed, but local main in $mainwt cannot fast-forward (its working tree is in the way); run: git -C $mainwt merge --ff-only origin/main"
  else git update-ref refs/heads/main "$tip"; fi
else die "pushed, but local main has commits that are not on origin/main ($(git rev-list --count "$tip"..main)); land them from a branch (scripts/land.sh), never merge into main"
fi
say "landed $br at $(git rev-parse --short "$tip"); scripts/main-build.sh status shows the build"
