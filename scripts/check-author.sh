#!/bin/sh
# Who may author and commit to Backplane: build@amemb.com (name i2cjak, every machine's own commits) and jlcjak's GitHub account
# (206324213+jlcjak@users.noreply.github.com, with GitHub, noreply@github.com, as committer of merges made on github.com). Nobody else.
# Fails, naming the commits, when a commit in the given rev-list arguments has another author or committer.
# usage: scripts/check-author.sh <rev-list args>   e.g. origin/main..HEAD   |   $new --not $old   (used by land.sh, the hooks and CI)
set -eu
# usage: scripts/check-author.sh --ident   checks the commit about to be made (git var, so env and config count): the pre-commit hook
[ $# -gt 0 ] || { echo "usage: check-author.sh <rev-list args> | --ident" >&2; exit 2; }
log() { if [ "$1" = --ident ]; then
    a=$(git var GIT_AUTHOR_IDENT | sed -n 's/.*<\(.*\)>.*/\1/p'); c=$(git var GIT_COMMITTER_IDENT | sed -n 's/.*<\(.*\)>.*/\1/p'); echo "this-commit $a $c"
  else git log --format='%h %ae %ce' "$@"; fi; }
want=build@amemb.com
bad=$(log "$@" | awk '
  function ok_author(e) { return e == "build@amemb.com" || e == "206324213+jlcjak@users.noreply.github.com" }
  function ok_committer(e) { return ok_author(e) || e == "noreply@github.com" }
  !ok_author($2) || !ok_committer($3) { print "  " $1 " author=" $2 " committer=" $3 }')
[ -z "$bad" ] && exit 0
echo "refused: commits by someone other than $want (i2cjak) or jlcjak:" >&2
echo "$bad" | head -20 >&2
cat >&2 <<HINT
fix: git config user.name i2cjak && git config user.email $want   (this repo's config; never the global one)
then re-author your own commits:  git rebase --exec 'git commit --amend --no-edit --reset-author' <base>
(also check GIT_AUTHOR_*/GIT_COMMITTER_* in the environment and any 'git -c user.email=' in your tooling)
HINT
exit 1
