#!/bin/bash
# scripts/land.sh against a local bare origin and a stand-in gh (fake-gh): the gate before the push, the branch and PR, the wait for a review (exit 3), the merge once it is
# approved (without a new gate while the tree is the tested one), the dry merge, a main that moved, a failing gate, --resume with no PR. No network, no GitHub.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/land-test.XXXXXX"); trap 'rm -rf "$T"' EXIT
export ORIGIN=$T/origin.git FAKE=$T/fake LAND_GH=$HERE/fake-gh LAND_REPO=x/y LAND_POLL_S=0 LAND_MIN_FREE_GB=0
mkdir -p "$FAKE"; fails=0
ok() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; fails=$((fails + 1)); fi; }
g() { git -C "$W" "$@"; }
export GIT_AUTHOR_NAME=i2cjak GIT_AUTHOR_EMAIL=build@amemb.com GIT_COMMITTER_NAME=i2cjak GIT_COMMITTER_EMAIL=build@amemb.com
git init -q --bare -b main "$ORIGIN"
W=$T/main; git clone -q "$ORIGIN" "$W" 2>/dev/null; g checkout -q -b main 2>/dev/null
mkdir -p "$W/scripts"; cp "$ROOT/scripts/land.sh" "$W/scripts/"; printf '#!/bin/sh\nexit 0\n' > "$W/scripts/check-author.sh"; chmod +x "$W/scripts/"*.sh
echo base > "$W/f"; g add -A; g commit -qm base; g push -q origin main
LANE=$T/lane; g worktree add -q -b lane "$LANE" 2>/dev/null; W=$LANE
echo one > "$LANE/doc"; g add -A; g commit -qm "docs: one"
land() { (cd "$LANE" && sh scripts/land.sh "$@" 2>&1); }
GATES=$T/gates; : > "$GATES"; export LAND_TEST="echo gate >> $GATES"
# 1. a failing gate pushes nothing and opens nothing
LAND_TEST=false land > "$T/o1"; rc=$?; [ $rc != 0 ] && ! git --git-dir="$ORIGIN" rev-parse -q --verify refs/heads/lane >/dev/null && [ ! -f "$FAKE/pr.lane" ]; ok $? "a failing gate: nothing pushed, no PR (rc $rc)"
# 2. --resume with no PR is refused
land --resume > "$T/o2"; rc=$?; [ $rc != 0 ] && grep -q "no open pull request" "$T/o2"; ok $? "--resume with no PR is refused"
# 3. gate green, PR blocked on review: exit 3 and the line
land > "$T/o3"; rc=$?; [ $rc = 3 ] && grep -q "PR #101 open, gate green on $(g rev-parse --short HEAD), waiting for an approving review" "$T/o3" && [ "$(wc -l < "$GATES")" = 1 ] && [ ! -f "$FAKE/merges" ]; ok $? "blocked on review: exit 3, the PR and the sha named, one gate, nothing merged (rc $rc)"
[ "$(git --git-dir="$ORIGIN" rev-parse refs/heads/lane)" = "$(g rev-parse HEAD)" ]; ok $? "the gated branch is on origin"
# 4. rerunning while still blocked does not gate again and opens no second PR
land > "$T/o4"; rc=$?; [ $rc = 3 ] && [ "$(wc -l < "$GATES")" = 1 ] && [ "$(cat "$FAKE/next")" = 101 ]; ok $? "rerun while blocked: exit 3, no new gate, no second PR"
# 5. approved: --dry-merge says what it would do and merges nothing
echo CLEAN > "$FAKE/mergestate"; echo APPROVED > "$FAKE/review"
land --resume --dry-merge > "$T/o5"; rc=$?; [ $rc = 0 ] && grep -q "would run: gh pr merge 101 --rebase --match-head-commit" "$T/o5" && [ ! -f "$FAKE/merges" ] && [ "$(wc -l < "$GATES")" = 1 ]; ok $? "dry merge: said, nothing merged, no gate (rc $rc)"
# 6. approved: --resume merges without a gate and fast-forwards main
LAND_TEST=false land --resume > "$T/o6"; rc=$?; tip=$(git --git-dir="$ORIGIN" rev-parse --short main)
[ $rc = 0 ] && [ "$(cat "$FAKE/merges")" = 101 ] && grep -q "landed lane (PR #101); main is $tip" "$T/o6" && [ "$(git -C "$T/main" rev-parse --short main)" = "$tip" ] && [ "$(git --git-dir="$ORIGIN" rev-parse main)" = "$(g rev-parse HEAD)" ]; ok $? "approved: merged without a gate, local main follows origin/main $tip (rc $rc)"
# 7. main moved while a PR waits: rebase, gate again, lease push, new head, blocked again
rm "$FAKE/mergestate" "$FAKE/review"; echo two > "$LANE/doc2"; g add -A; g commit -qm "docs: two"
land > "$T/o7a"; rc=$?; [ $rc = 3 ]; ok $? "second commit: new PR waits (rc $rc)"
M=$T/mover; git clone -q "$ORIGIN" "$M" 2>/dev/null; echo moved > "$M/m"; git -C "$M" add -A; git -C "$M" commit -qm "main moved"; git -C "$M" push -q origin main
before=$(wc -l < "$GATES"); land > "$T/o7b"; rc=$?; [ $rc = 3 ] && [ "$(wc -l < "$GATES")" = $((before + 1)) ] && [ "$(git --git-dir="$ORIGIN" rev-parse refs/heads/lane)" = "$(g rev-parse HEAD)" ] && g merge-base --is-ancestor "$(git --git-dir="$ORIGIN" rev-parse main)" HEAD; ok $? "main moved: rebased on it, gated again, branch force-pushed with lease, waiting again (rc $rc)"
# 8. a branch with a PR whose mergeability says BEHIND is retried up to LAND_TRIES, then fails
echo BEHIND > "$FAKE/mergestate"; LAND_TRIES=2 land > "$T/o8"; rc=$?; [ $rc = 1 ] && grep -q "still BEHIND after 2 tries" "$T/o8"; ok $? "BEHIND forever: fails after LAND_TRIES (rc $rc)"
# 9. no gh login for i2cjak
echo BLOCKED > "$FAKE/mergestate"; FAKE_NOLOGIN=1 land > "$T/o9"; rc=$?; [ $rc != 0 ] && grep -q "no gh login for i2cjak" "$T/o9"; ok $? "no i2cjak login: refused, the active account is not used (rc $rc)"
# 9b. blocked with no review asked for: said as that, exit 3
echo NONE > "$FAKE/review"; land > "$T/o9b"; rc=$?; [ $rc = 3 ] && grep -q "blocked by a rule with no review asked for" "$T/o9b"; ok $? "blocked, no review decision: its own line, exit 3 (rc $rc)"
# 10. the token is never printed
! grep -q fake-token "$T"/o[0-9]*; ok $? "the token is never printed"
# 11. a stale copy of land.sh (main's has changed since this lane's) refuses to run; a copy ahead of main's (a change being landed) does not
git -C "$M" pull -q origin main 2>/dev/null; echo "# newer" >> "$M/scripts/land.sh"; git -C "$M" commit -qam "land.sh: newer"; git -C "$M" push -q origin main
before=$(wc -l < "$GATES"); land > "$T/o11"; rc=$?; [ $rc = 1 ] && grep -q "older than origin/main's" "$T/o11" && [ "$(wc -l < "$GATES")" = "$before" ]; ok $? "a stale land.sh refuses to run (rc $rc)"
git -C "$LANE" rebase -q origin/main 2>/dev/null || { git -C "$LANE" fetch -q origin main; git -C "$LANE" rebase -q origin/main; }; echo "# ahead" >> "$LANE/scripts/land.sh"; g commit -qam "land.sh: ahead"
land > "$T/o11b"; ! grep -q "older than origin/main's" "$T/o11b"; ok $? "a land.sh ahead of main's runs"
# 12. nothing ever pushes to refs/heads/main itself
! grep -n 'push.*refs/heads/main\|push origin HEAD:main\|push.*HEAD:main' "$ROOT/scripts/land.sh" | grep -v '^[0-9]*:#'; ok $? "land.sh has no push to main"
# 13. an edit of land.sh while a land runs (here from its gate) changes nothing in that run: it ends as it would have, not on a syntax error
echo three > "$LANE/doc3"; g add -A; g commit -qm "docs: three"
LAND_TEST="printf '\n( ((\n' >> $LANE/scripts/land.sh" land > "$T/o13"; rc=$?; g checkout -q scripts/land.sh
[ $rc = 3 ] && ! grep -qi "syntax error" "$T/o13" && grep -q "waiting for an approving review\|blocked by a rule" "$T/o13"; ok $? "land.sh edited under a running land: the run is unchanged (rc $rc)"
! ls "${TMPDIR:-/tmp}"/land-run.* >/dev/null 2>&1; ok $? "the private copy is removed"
# 14. a token that is not i2cjak's on GitHub: no pull request is opened or merged
echo four > "$LANE/doc4"; g add -A; g commit -qm "docs: four"
echo CLEAN > "$FAKE/mergestate"; echo APPROVED > "$FAKE/review"; nm=$(wc -l < "$FAKE/merges"); FAKE_LOGIN=hlord2000 land > "$T/o14"; rc=$?; [ $rc != 0 ] && grep -q "not i2cjak" "$T/o14" && [ "$(wc -l < "$FAKE/merges")" = "$nm" ]; ok $? "a token that is another account's is refused before a PR (rc $rc)"
# 15. a lane commit not made as i2cjak <build@amemb.com> is refused, with the fix
echo five > "$LANE/doc5"; g add -A; GIT_AUTHOR_EMAIL=x@y.org g commit -qm "docs: five"
land > "$T/o15"; rc=$?; [ $rc != 0 ] && grep -q "not made as i2cjak <build@amemb.com>" "$T/o15" && grep -q "reset-author" "$T/o15"; ok $? "a commit by another identity is refused, with the fix (rc $rc)"
[ $fails = 0 ] && echo "land_pr_test: all passed" || { echo "land_pr_test: $fails failed"; exit 1; }
