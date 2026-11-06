#!/bin/sh
# land.sh TIP: fast-forward main to TIP under the gpui-merge lock, if TIP sits on main and the
# reviewer OKed every commit in main..TIP (scripts/gpui/review-ok.sh). Run it from your own
# worktree with your branch rebased on main. Nothing else can move main (ref-guard.hook).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
if [ "${GPUI_LAND_LOCKED:-}" != 1 ]; then
  GPUI_LAND_LOCKED=1 exec "${GPUI_LAND_LOCK:-$here/../bp-lock.sh}" gpui-merge -- "$here/land.sh" "$@"
fi
tip=$(git rev-parse --verify "${1:?usage: land.sh TIP}^{commit}")
c=$(cd "$(git rev-parse --git-common-dir)" && pwd)
git merge-base --is-ancestor main "$tip" || { echo "refused: $tip is not on main; rebase first" >&2; exit 1; }
[ -z "$(git rev-list --merges "main..$tip")" ] || { echo "refused: merge commits in the range" >&2; exit 1; }
bad=0; n=0
nr=; [ -e "$c/gpui-no-review" ] && nr=1
for s in $(git rev-list --reverse "main..$tip"); do
  n=$((n+1)); [ -n "$nr" ] && continue; p=$(git diff-tree -p "$s" | git patch-id --stable | cut -d' ' -f1)
  if grep -q "^$s " "$c/gpui-reviewed" 2>/dev/null; then continue; fi
  if [ -n "$p" ] && grep -q "^[0-9a-f]* $p " "$c/gpui-reviewed" 2>/dev/null; then continue; fi
  echo "not reviewed: $(git log -1 --format='%h %s' "$s")" >&2; bad=1
done
# nothing over 1 MB and no build output (the bundled fonts are the one exception: COMMON.md)
big=$(git rev-list --objects "main..$tip" | git cat-file --batch-check='%(objecttype) %(objectsize) %(rest)' | awk '$1=="blob" && $3!="" && ($2>1000000 || $3 ~ /(^|\/)target\//) && $3 !~ /\.ttf$/ {print $2, $3}')
if [ -n "$big" ]; then echo "$big" | head -20 >&2; echo "refused: files over 1 MB or build output in the range" >&2; bad=1; fi
# the person's goal is theirs: a lane's range never changes it
if git diff --name-only "main..$tip" | grep -qx '.backplane/intent.md'; then echo "refused: .backplane/intent.md changes only on the person's word" >&2; bad=1; fi
[ "$bad" = 0 ] || { echo "refused: nothing moved" >&2; exit 1; }
[ "$n" -gt 0 ] || { echo "nothing to land"; exit 0; }
w=$(git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch refs\/heads\/main$/{print w; exit}')
tok=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n'); (umask 077; printf '%s' "$tok" > "$c/gpui-land.token")
trap 'rm -f "$c/gpui-land.token"' EXIT
if [ -n "$w" ]; then GPUI_LAND=$tok git -C "$w" merge -q --ff-only "$tip"
else GPUI_LAND=$tok git update-ref refs/heads/main "$tip" "$(git rev-parse main)"; fi
echo "landed $n commits: main is $(git rev-parse --short main)"
