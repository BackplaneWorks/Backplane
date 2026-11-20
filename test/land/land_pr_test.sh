#!/bin/bash
# scripts/land.sh against a local bare origin and a stand-in gh (fake-gh): the gate before the push, the branch and PR, the wait for a review (exit 3), the merge once it is
# approved (without a new gate while the tree is the tested one), the dry merge, a main that moved, a failing gate, --resume with no PR. No network, no GitHub.
set -u
# run from inside a land (its gate), the outer land's private copy must not leak into the lands this test starts
unset LAND_COPY LAND_HERE LAND_TEST LAND_WIN_TEST GH_TOKEN
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/land-test.XXXXXX"); trap '[ -n "${KEEP:-}" ] || rm -rf "$T"' EXIT
# the land runs' own temporary folder (their private copies, land-run.*, are looked for here alone), and the post-merge log
export TMPDIR=$T/tmp LAND_POSTMERGE_LOG=$T/pm/postmerge.log; mkdir -p "$TMPDIR"
export ORIGIN=$T/origin.git FAKE=$T/fake LAND_GH=$HERE/fake-gh LAND_REPO=x/y LAND_POLL_S=0 LAND_MIN_FREE_GB=0
# the Windows gate's lock: no hub answers (a closed loopback port), so scripts/bp-lock.sh takes a flock in $T/locks
export BACKPLANE_LOCK_URL=http://127.0.0.1:9 BACKPLANE_LOCK_HOME=$T/nohub BACKPLANE_LOCK_DIR=$T/locks; mkdir -p "$T/locks"
WINLOCK=$T/locks/bp-bp-win-gate.lock
# the Windows gate's shutdown check off by default here (no measured time, no margin); the cases below set a VM's measured time in $T/state
export LAND_WIN_STATE=$T/state LAND_WIN_EST_MIN=0 LAND_WIN_MARGIN_MIN=0
# a stand-in az for every case (never the real one): each VM's power in $T/power.<vm> (running unless said), every call logged to $AZLOG;
# bp-winbuild's public address is 203.0.113.7, and this machine's 198.51.100.1
export AZLOG=$T/azlog LAND_WIN_AZ=$T/azf LAND_WIN_MYIP=198.51.100.1 POWER=$T/power
# never a real VM from here: ssh is a stand-in that reaches nothing unless a case gives its own, and the post-merge gate is off unless a case turns it on
printf '#!/bin/sh\nexit 255\n' > "$T/nossh"; chmod +x "$T/nossh"; export LAND_WIN_SSH=$T/nossh LAND_POSTMERGE=off
cat > "$T/azf" <<'EOS'
#!/bin/sh
n=; a="$*"; while [ $# -gt 0 ]; do [ "$1" = -n ] && n=$2; shift; done
echo "$a" | sed 's/ -o [a-z]*//; s/ --query .*//' >> "$AZLOG"
case "$a" in
  *get-instance-view*) echo "PowerState/$(cat "$POWER.$n" 2>/dev/null || echo running)";;
  "vm start"*) echo running > "$POWER.$n";;
  "vm deallocate"*) echo deallocated > "$POWER.$n";;
  "network public-ip show"*) echo 203.0.113.7;;
  "network nsg rule create"*|"network nsg rule delete"*) ;;
  *) echo "az stand-in: unhandled: $a" >&2; exit 2;;
esac
EOS
chmod +x "$T/azf"
mkdir -p "$FAKE"; fails=0
ok() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; fails=$((fails + 1)); fi; }
g() { git -C "$W" "$@"; }
export GIT_AUTHOR_NAME=i2cjak GIT_AUTHOR_EMAIL=build@amemb.com GIT_COMMITTER_NAME=i2cjak GIT_COMMITTER_EMAIL=build@amemb.com
git init -q --bare -b main "$ORIGIN"
W=$T/main; git clone -q "$ORIGIN" "$W" 2>/dev/null; g checkout -q -b main 2>/dev/null
mkdir -p "$W/scripts"; cp "$ROOT/scripts/land.sh" "$ROOT/scripts/land-windows.sh" "$ROOT/scripts/bp-lock.sh" "$ROOT/scripts/hub-outside-files.py" "$ROOT/scripts/land-gate.sh" "$W/scripts/"; printf '#!/bin/sh\nexit 0\n' > "$W/scripts/check-author.sh"; chmod +x "$W/scripts/"*.sh
echo base > "$W/f"; g add -A; g commit -qm base; g push -q origin main
LANE=$T/lane; g worktree add -q -b lane "$LANE" 2>/dev/null; W=$LANE
echo one > "$LANE/doc"; g add -A; g commit -qm "docs: one"
land() { (cd "$LANE" && sh scripts/land.sh "$@" 2>&1); }
GATES=$T/gates; : > "$GATES"; export LAND_TEST="echo gate >> $GATES"
WINS=$T/wins; : > "$WINS"; export LAND_WIN_TEST="echo win >> $WINS"
# 1. a failing gate pushes nothing and opens nothing
LAND_TEST=false land > "$T/o1"; rc=$?; [ $rc != 0 ] && ! git --git-dir="$ORIGIN" rev-parse -q --verify refs/heads/lane >/dev/null && [ ! -f "$FAKE/pr.lane" ]; ok $? "a failing gate: nothing pushed, no PR (rc $rc)"
# 2. --resume with no PR is refused
land --resume > "$T/o2"; rc=$?; [ $rc != 0 ] && grep -q "no open pull request" "$T/o2"; ok $? "--resume with no PR is refused"
# 3. gate green, PR blocked on review: exit 3 and the line
land > "$T/o3"; rc=$?; [ $rc = 3 ] && grep -q "PR #101 open, gate green on $(g rev-parse --short HEAD), waiting for an approving review" "$T/o3" && [ "$(wc -l < "$GATES")" = 1 ] && [ ! -f "$FAKE/merges" ]; ok $? "blocked on review: exit 3, the PR and the sha named, one gate, nothing merged (rc $rc)"
! grep -q -i "windows" "$T/o3" && [ ! -s "$WINS" ]; ok $? "no Windows gate at a merge: none run, none mentioned"
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
# 16. main moves between the gate and the merge while the PR still reads mergeable: no merge of a tree the gate never ran (#1374); it rebases, gates
#     again on the new main and merges that
g reset -q --hard HEAD~1; git -C "$LANE" fetch -q origin main; g rebase -q origin/main; echo six > "$LANE/doc6"; g add -A; g commit -qm "docs: six"
echo CLEAN > "$FAKE/mergestate"; echo APPROVED > "$FAKE/review"; rm -f "$T/moved"; before=$(wc -l < "$GATES")
LAND_TEST="echo gate >> $GATES; [ -f $T/moved ] || { touch $T/moved; git -C $M pull -q origin main 2>/dev/null; echo m2 > $M/m2; git -C $M add -A; git -C $M commit -qm 'main moved during the gate'; git -C $M push -q origin main; }" land > "$T/o16"; rc=$?
mover=$(git -C "$M" rev-parse HEAD)
[ $rc = 0 ] && [ "$(wc -l < "$GATES")" = $((before + 2)) ] && grep -q "main moved after the gate" "$T/o16" && git --git-dir="$ORIGIN" merge-base --is-ancestor "$mover" main; ok $? "main moved after the gate: gated again on the new main, then merged; the moved commit is kept (rc $rc)"
g reset -q --hard HEAD~1; rm -f "$FAKE/mergestate" "$FAKE/review"
pushed() { [ "$(git --git-dir="$ORIGIN" rev-parse -q --verify refs/heads/lane)" = "$(g rev-parse HEAD)" ]; }
# 17. no Windows gate at a merge: a host PR merges on the gate alone (LAND_WIN_TEST, set above, is never run)
echo CLEAN > "$FAKE/mergestate"; echo APPROVED > "$FAKE/review"; g fetch -q origin main; g reset -q --hard origin/main; : > "$WINS"
mkdir -p "$LANE/native/host/src"; echo 'fn a() {}' > "$LANE/native/host/src/a.rs"; g add -A; g commit -qm "host: a"
land > "$T/o17"; rc=$?; [ $rc = 0 ] && [ ! -s "$WINS" ] && grep -q "landed lane" "$T/o17" && ! grep -q -i "windows gate" "$T/o17"; ok $? "a host PR merges without any Windows gate (rc $rc)"
# 18-22. the real gate (scripts/land-gate.sh) with a stand-in cargo: what each PR compiles and tests
printf '#!/bin/sh\necho "$(basename "$PWD") $*" >> %s\ncase "$*" in *FAILME*) exit 1;; esac\n[ -f "$PWD/FAIL" ] && exit 1\nexit 0\n' "$T/cargolog" > "$T/cargo"; chmod +x "$T/cargo"
gland() { : > "$T/cargolog"; (cd "$LANE" && LAND_GATE_CARGO=$T/cargo sh -c 'unset LAND_TEST; sh scripts/land.sh' 2>&1); }
mkdir -p "$LANE/docs" "$LANE/mobile" "$LANE/web/src"; echo d > "$LANE/docs/x.md"; echo m > "$LANE/mobile/m.kt"; echo w > "$LANE/web/src/a.ts"; echo s > "$LANE/scripts/s.sh"; g add -A; g commit -qm "docs, mobile, web sources, scripts"
gland > "$T/o18"; rc=$?; [ $rc = 0 ] && [ ! -s "$T/cargolog" ] && grep -q "nothing to compile: the PR touches no crate" "$T/o18" && grep -q "gate passed in [0-9]*s" "$T/o18"; ok $? "docs/mobile/web sources/scripts: nothing compiled, landed (rc $rc)"
echo 'fn b() {}' > "$LANE/native/host/src/b.rs"; g add -A; g commit -qm "host: b"
gland > "$T/o19"; rc=$?; [ $rc = 0 ] && [ "$(cut -d' ' -f1-3 "$T/cargolog" | tr '\n' '|')" = "host check --all-targets|host test --locked|" ] && grep -q -- "-- --skip e2e" "$T/cargolog" && grep -q "gate: passed in .*host: cargo check --all-targets [0-9]*s, host: unit tests [0-9]*s" "$T/o19"; ok $? "a host PR: cargo check and the unit tests (no e2e) of the host alone, timed (rc $rc)"
mkdir -p "$LANE/web/dist"; echo '<html>1' > "$LANE/web/dist/index.html"; printf '%s\n' 'macro_rules! asset { ($p:literal) => { include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/dist/", $p)) } }' > "$LANE/native/host/src/webingress.rs"; g add -A; g commit -qm "host: embeds web/dist"; gland > /dev/null
echo '<html>2' > "$LANE/web/dist/index.html"; g commit -qam "web/dist only"
gland > "$T/o20"; rc=$?; [ $rc = 0 ] && grep -q '^host check' "$T/cargolog" && ! grep -q '^gpui' "$T/cargolog"; ok $? "a web/dist-only PR checks and tests the host, which embeds it (rc $rc)"
mkdir -p "$LANE/native/gpui/src"; echo 'fn g() {}' > "$LANE/native/gpui/src/g.rs"; g add -A; g commit -qm "gpui: g"
gland > "$T/o21"; rc=$?; [ $rc = 0 ] && [ "$(cut -d' ' -f1 "$T/cargolog" | sort -u)" = gpui ]; ok $? "a gpui PR: the gpui crate alone (rc $rc)"
touch "$LANE/native/host/FAIL"; echo 'fn c() {}' > "$LANE/native/host/src/c.rs"; g add -A; g commit -qm "host: c, failing"; before=$(git --git-dir="$ORIGIN" rev-parse -q --verify refs/heads/lane)
gland > "$T/o22"; rc=$?; [ $rc != 0 ] && grep -q "gate: FAILED" "$T/o22" && grep -q "the gate failed (above); nothing pushed" "$T/o22" && [ "$(git --git-dir="$ORIGIN" rev-parse -q --verify refs/heads/lane)" = "$before" ]; ok $? "a failing check: the gate fails, nothing pushed (rc $rc)"
[ $fails = 0 ] && echo "land_pr_test: all passed" || { echo "land_pr_test: $fails failed"; exit 1; }
