#!/bin/sh
# next-land.sh TIP: fast-forward gpui-next (the branch every merge lands on first, and the one the builder follows) to TIP, under the gpui-next-merge lock.
# Run it from your own worktree with your branch rebased on gpui-next. gpui-next is not the guarded main: no review is asked, and the builder's gate (the tests) is what stands
# between a commit and the app. Promote a reviewed gpui-next to main with land.sh.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
if [ "${GPUI_NEXT_LOCKED:-}" != 1 ]; then GPUI_NEXT_LOCKED=1 exec "$here/../bp-lock.sh" gpui-next-merge -- "$here/next-land.sh" "$@"; fi
tip=$(git rev-parse --verify "${1:?usage: next-land.sh TIP}^{commit}")
git rev-parse --verify -q refs/heads/gpui-next >/dev/null || { echo "no gpui-next branch: git branch gpui-next main" >&2; exit 1; }
git merge-base --is-ancestor gpui-next "$tip" || { echo "refused: $tip is not on gpui-next; rebase first (git rebase gpui-next)" >&2; exit 1; }
[ "$(git rev-parse gpui-next)" != "$tip" ] || { echo "nothing to land"; exit 0; }
[ -z "$(git rev-list --merges "gpui-next..$tip")" ] || { echo "refused: merge commits in the range" >&2; exit 1; }
big=$(git rev-list --objects "gpui-next..$tip" | git cat-file --batch-check='%(objecttype) %(objectsize) %(rest)' | awk '$1=="blob" && $3!="" && ($2>1000000 || $3 ~ /(^|\/)target\//) && $3 !~ /\.ttf$/ {print $2, $3}')
[ -z "$big" ] || { echo "$big" | head -10 >&2; echo "refused: files over 1 MB or build output in the range" >&2; exit 1; }
n=$(git rev-list --count "gpui-next..$tip")
w=$(git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch refs\/heads\/gpui-next$/{print w; exit}')
if [ -n "$w" ]; then git -C "$w" merge -q --ff-only "$tip"; else git update-ref refs/heads/gpui-next "$tip" "$(git rev-parse gpui-next)"; fi
echo "landed $n commits: gpui-next is $(git rev-parse --short gpui-next); the builder picks it up"
