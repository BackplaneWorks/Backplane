#!/bin/bash
# Identity matrix and installed hooks in scratch repositories; no network or real user data.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh"
T=$(mktemp -d "$BP_TMP/check-author-test.XXXXXX"); trap 'rm -rf "$T"' EXIT
fails=0; checks=0
ok() { checks=$((checks + 1)); if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; fails=$((fails + 1)); fi; }
export GIT_AUTHOR_NAME=test GIT_COMMITTER_NAME=test
export GIT_AUTHOR_EMAIL=build@amemb.com GIT_COMMITTER_EMAIL=build@amemb.com
git init -q -b main "$T/r"; cd "$T/r"
mk() { GIT_AUTHOR_EMAIL=$1 GIT_COMMITTER_EMAIL=$2 git -c core.hooksPath=/dev/null commit -q --allow-empty -m "$1/$2"; }
chk() { sh "$ROOT/scripts/check-author.sh" "$@" >"$T/check-output" 2>&1; }
mk unknown@invalid.example unknown@invalid.example; base=$(git rev-parse HEAD)
# Every allowed combination, with a rejected ancestor outside the checked range.
people=(build@amemb.com 231940135+i2cjak@users.noreply.github.com 206324213+jlcjak@users.noreply.github.com kpatsatzakis1@gmail.com)
for a in "${people[@]}"; do
  for c in "${people[@]}" noreply@github.com; do
    mk "$a" "$c"
    if chk "$base..HEAD" && chk HEAD --not "$base" && GIT_AUTHOR_EMAIL=$a GIT_COMMITTER_EMAIL=$c chk --ident; then rc=0; else rc=1; fi
    ok "$rc" "range and --ident accept $a / $c"
  done
done
good=$(git rev-parse HEAD)
if ! chk HEAD; then rc=0; else rc=1; fi; ok "$rc" "full history still rejects the unknown ancestor"
for a in noreply@github.com unknown@invalid.example; do
  mk "$a" build@amemb.com
  if ! chk "$good..HEAD" && ! GIT_AUTHOR_EMAIL=$a chk --ident; then rc=0; else rc=1; fi
  ok "$rc" "range and --ident reject author $a"; git reset -q --hard "$good"
done
mk build@amemb.com unknown@invalid.example
if ! chk "$good..HEAD" && ! GIT_COMMITTER_EMAIL=unknown@invalid.example chk --ident; then rc=0; else rc=1; fi
ok "$rc" "range and --ident reject an unknown committer"
if ! grep -q 'reset-author\|re-author' "$T/check-output"; then rc=0; else rc=1; fi; ok "$rc" "hint preserves accepted identities"
if chk "$good..$good" && chk "$good" --not "$good"; then rc=0; else rc=1; fi; ok "$rc" "an empty range (no new commits) passes"
if ! chk nonexistent-ref; then rc=0; else rc=1; fi; ok "$rc" "an invalid revision fails closed"
git reset -q --hard "$good"
# Install from a lane, while the main checkout's old checker rejects everything.
mkdir scripts
printf '#!/bin/sh\nexit 1\n' > scripts/check-author.sh; chmod +x scripts/check-author.sh
printf '#!/bin/sh\nexit 0\n' > scripts/main-build.sh; chmod +x scripts/main-build.sh
git add scripts; git commit -qm 'stale main checker'; old=$(git rev-parse HEAD)
git update-ref refs/remotes/origin/main "$old"
git worktree add -q -b lane "$T/lane"
cd "$T/lane"
mkdir -p scripts; cp "$ROOT/scripts/"{check-author.sh,install-hooks.sh,pre-commit.hook,pre-push.hook,main.hook} scripts/
sh scripts/install-hooks.sh >"$T/install-output"
hooks=$(git rev-parse --git-common-dir)/hooks
cmp scripts/check-author.sh "$hooks/check-author.sh"; ok $? "hook installer bundles the checker in the common git directory"
git add scripts
for a in "${people[@]}"; do
  if GIT_AUTHOR_EMAIL=$a GIT_COMMITTER_EMAIL=$a git commit -qm "accepted $a" --allow-empty; then rc=0; else rc=1; fi
  ok "$rc" "installed pre-commit accepts $a despite stale main checkout"
done
if ! GIT_AUTHOR_EMAIL=noreply@github.com git commit -qm rejected --allow-empty >"$T/rejected" 2>&1; then rc=0; else rc=1; fi
ok "$rc" "installed pre-commit rejects GitHub as author"
git remote add origin "$T/r"
tip=$(git rev-parse HEAD)
if printf 'refs/heads/lane %s refs/heads/lane %s\n' "$tip" "$old" | "$hooks/pre-push"; then rc=0; else rc=1; fi
ok "$rc" "installed pre-push checks only commits absent from remotes"
git tag -a -m tag vtest "$old"
if printf 'refs/tags/vtest %s refs/tags/vtest %s\n' "$(git rev-parse vtest)" 0000000000000000000000000000000000000000 | "$hooks/pre-push"; then rc=0; else rc=1; fi
ok "$rc" "installed pre-push accepts a new tag on a commit already on the remote"
# Real ref transactions exercise new-not-old, followed by local main's fast-forward.
if git update-ref refs/remotes/origin/main "$tip" "$old" && git -C "$T/r" merge -q --ff-only origin/main; then rc=0; else rc=1; fi
ok "$rc" "origin/main and local main advance with accepted commits and a rejected old ancestor"
# Rebase a lane over a main commit whose GitHub committer used to be rejected.
git -C "$T/r" checkout -q -b mover
(cd "$T/r" && mk build@amemb.com 231940135+i2cjak@users.noreply.github.com)
upstream=$(git -C "$T/r" rev-parse HEAD)
git update-ref refs/remotes/origin/main "$upstream" "$tip"
git commit -qm 'lane own commit' --allow-empty
if git rebase -q origin/main && sh scripts/check-author.sh origin/main..HEAD; then rc=0; else rc=1; fi
ok "$rc" "lane rebases over GitHub-committed main and checks only origin/main..HEAD"
mk build@amemb.com unknown@invalid.example; rejected=$(git rev-parse HEAD)
if ! printf 'refs/heads/lane %s refs/heads/lane %s\n' "$rejected" "$old" | "$hooks/pre-push" >"$T/rejected" 2>&1; then rc=0; else rc=1; fi
ok "$rc" "installed pre-push rejects a new unknown identity"
if ! git update-ref refs/remotes/origin/main "$rejected" "$upstream" >"$T/rejected" 2>&1 && [ "$(git rev-parse origin/main)" = "$upstream" ]; then rc=0; else rc=1; fi
ok "$rc" "reference transaction rejects a new unknown identity without moving the ref"
# Local main may still only follow origin/main, even for accepted identities.
git reset -q --hard "$upstream"; mk build@amemb.com build@amemb.com
if ! git update-ref refs/heads/main HEAD "$tip" >"$T/rejected" 2>&1 && grep -q 'not on origin/main' "$T/rejected"; then rc=0; else rc=1; fi
ok "$rc" "local main cannot advance past origin/main"
[ "$fails" = 0 ] && echo "check_author_test: $checks passed" || { echo "check_author_test: $fails/$checks failed"; exit 1; }
