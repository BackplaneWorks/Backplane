#!/bin/sh
# Installed by scripts/install-hooks.sh alongside the hooks so stale worktrees use the updated rule.
# i2cjak and jlcjak each have two accepted addresses, as author and committer.
# GitHub's rebase-merge of a PR by i2cjak keeps the author and sets the committer to
# 231940135+i2cjak@users.noreply.github.com. GitHub itself is accepted as committer only.
# Fails, naming the commits, when a commit in the given rev-list arguments has another author or committer.
# usage: scripts/check-author.sh <rev-list args>   e.g. origin/main..HEAD   |   $new --not $old   (used by land.sh, the hooks and CI)
set -eu
# usage: scripts/check-author.sh --ident   checks the commit about to be made (git var, so env and config count): the pre-commit hook
[ $# -gt 0 ] || { echo "usage: check-author.sh <rev-list args> | --ident" >&2; exit 2; }
# Commits already on the remote's main (refs/remotes/origin/main, or CHECK_AUTHOR_UPSTREAM; "none" for no exclusion) are never checked: they
# are published history, whoever wrote them (the person's decision, Nov 19). Only new commits being made, landed or pushed are.
up=${CHECK_AUTHOR_UPSTREAM:-refs/remotes/origin/main}
if [ "$1" != --ident ] && [ "$up" != none ] && git rev-parse -q --verify "$up^{commit}" > /dev/null 2>&1; then
  # each --not flips what follows: after an odd number of them (`$new --not $old`) a plain ref is already an exclusion
  n=0; for a in "$@"; do [ "$a" = --not ] && n=$((n + 1)); done
  if [ $((n % 2)) = 1 ]; then set -- "$@" "$up"; else set -- "$@" --not "$up"; fi
fi
log() { if [ "$1" = --ident ]; then
    a=$(git var GIT_AUTHOR_IDENT | sed -n 's/.*<\(.*\)>.*/\1/p'); c=$(git var GIT_COMMITTER_IDENT | sed -n 's/.*<\(.*\)>.*/\1/p'); echo "this-commit $a $c"
  else git log --format='%h %ae %ce' "$@"; fi; }
entries=$(log "$@") || exit $?
bad=$(printf '%s\n' "$entries" | awk '
  function ok_author(e) { return e == "build@amemb.com" || e == "231940135+i2cjak@users.noreply.github.com" || e == "206324213+jlcjak@users.noreply.github.com" || e == "kpatsatzakis1@gmail.com" }
  function ok_committer(e) { return ok_author(e) || e == "noreply@github.com" }
  NF == 0 { next }  # an empty range (a push of a tag on a commit already on the remote) is one empty line
  !ok_author($2) || !ok_committer($3) { print "  " $1 " author=" $2 " committer=" $3 }')
[ -z "$bad" ] && exit 0
echo "refused: unapproved author or committer address:" >&2
echo "$bad" | head -20 >&2
cat >&2 <<'HINT'
accepted authors and committers:
  i2cjak: build@amemb.com, 231940135+i2cjak@users.noreply.github.com
  jlcjak: 206324213+jlcjak@users.noreply.github.com, kpatsatzakis1@gmail.com
  committer only: noreply@github.com
Check this repo's user.name/user.email, GIT_AUTHOR_*/GIT_COMMITTER_* and tooling overrides.
Correct only an unapproved identity on your own unpublished commits; preserve accepted identities and existing history.
HINT
exit 1
