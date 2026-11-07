#!/bin/sh
# Every commit of Backplane has author AND committer build@amemb.com (name i2cjak). Fails, naming the commits, when a commit in the given rev-list arguments has another email.
# usage: scripts/check-author.sh <rev-list args>   e.g. origin/main..HEAD   |   $new --not $old   (used by land.sh, the hooks and CI)
set -eu
[ $# -gt 0 ] || { echo "usage: check-author.sh <rev-list args>" >&2; exit 2; }
want=build@amemb.com
bad=$(git log --format='%h %ae %ce %s' "$@" | awk -v w="$want" '$2 != w || $3 != w { print "  " $1 " author=" $2 " committer=" $3 }')
[ -z "$bad" ] && exit 0
echo "refused: commits not authored and committed by $want:" >&2
echo "$bad" | head -20 >&2
cat >&2 <<HINT
fix: git config user.name i2cjak && git config user.email $want   (this repo's config; never the global one)
then re-author your own commits:  git rebase --exec 'git commit --amend --no-edit --reset-author' <base>
(also check GIT_AUTHOR_*/GIT_COMMITTER_* in the environment and any 'git -c user.email=' in your tooling)
HINT
exit 1
