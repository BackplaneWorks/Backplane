#!/bin/sh
# Lands the branch checked out here on main, refusing any commit whose author or committer is not build@amemb.com.
# usage: scripts/land.sh [--push]
#   1. checks origin/main..HEAD (scripts/check-author.sh), and that HEAD is on top of main (rebase first);
#   2. fast-forwards main from the checkout that has it (never forces, never a merge commit);
#   3. with --push: checks origin/main..main again and pushes main as a fast-forward (only when the person asked for a push).
# The tests are yours to run first (scripts/dev-build.sh --gate --no-cut).
set -eu
cd "$(git rev-parse --show-toplevel)"
here=$(dirname "$0")
git fetch -q origin main
"$here/check-author.sh" origin/main..HEAD
git merge-base --is-ancestor main HEAD || { echo "refused: HEAD is not on top of main; git rebase main first" >&2; exit 1; }
mainwt=$(git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch refs\/heads\/main$/{print w}')
[ -n "$mainwt" ] || { echo "refused: no checkout has main" >&2; exit 1; }
git -C "$mainwt" merge --ff-only "$(git rev-parse HEAD)"
if [ "${1:-}" = --push ]; then
  "$here/check-author.sh" origin/main..main
  git merge-base --is-ancestor origin/main main || { echo "refused: origin/main has moved; rebase and land again (never force)" >&2; exit 1; }
  git push origin "$(git rev-parse main):refs/heads/main"
fi
