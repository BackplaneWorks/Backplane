#!/bin/sh
# review-ok.sh BASE..TIP: the reviewer's OK, one line per commit in <common .git>/gpui-reviewed
# (hash, patch id, when, session), read by land.sh. The patch id lets an OK survive a clean
# rebase; a rebase that changes a commit's diff needs a new OK. `patch-id --stable` ignores
# whitespace, so a whitespace-only change keeps the OK. Only the enrolled reviewer
# session may write. `review-ok.sh --enrol` enrols once; the coordinator deletes
# <common .git>/gpui-reviewer to enrol another.
set -eu
c=$(git rev-parse --git-common-dir); who=${CLAUDE_CODE_SESSION_ID:-}
[ -n "$who" ] || { echo "no session id" >&2; exit 1; }
if [ "${1:-}" = --enrol ]; then
  [ ! -e "$c/gpui-reviewer" ] || { echo "already enrolled" >&2; exit 1; }
  printf '%s\n' "$who" > "$c/gpui-reviewer"; echo enrolled; exit 0
fi
[ "$who" = "$(cat "$c/gpui-reviewer" 2>/dev/null)" ] || { echo "only the reviewer records an OK" >&2; exit 1; }
case "${1:-}" in *...*) echo "usage: review-ok.sh BASE..TIP (two dots)" >&2; exit 2 ;; *..*) ;; *) echo "usage: review-ok.sh BASE..TIP" >&2; exit 2 ;; esac
list=$(git rev-list --reverse "$1") || { echo "bad range: $1" >&2; exit 1; }
[ -n "$list" ] || { echo "empty range: $1" >&2; exit 1; }
n=0
for s in $list; do
  p=$(git diff-tree -p "$s" | git patch-id --stable | cut -d' ' -f1)
  printf '%s %s %s %s\n' "$s" "${p:--}" "$(date -u +%FT%TZ)" "$who" >> "$c/gpui-reviewed"; n=$((n+1))
done
echo "recorded $n commits"
