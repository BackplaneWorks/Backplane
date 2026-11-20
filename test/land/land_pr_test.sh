#!/bin/bash
# scripts/land.sh against a local bare origin and a stand-in gh (fake-gh): the gate before the push, the branch and PR, the wait for a review (exit 3), the merge once it is
# approved (without a new gate while the tree is the tested one), the dry merge, a main that moved, a failing gate, --resume with no PR. No network, no GitHub.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/land-test.XXXXXX"); trap '[ -n "${KEEP:-}" ] || rm -rf "$T"' EXIT
export ORIGIN=$T/origin.git FAKE=$T/fake LAND_GH=$HERE/fake-gh LAND_REPO=x/y LAND_POLL_S=0 LAND_MIN_FREE_GB=0
# the Windows gate's lock: no hub answers (a closed loopback port), so scripts/bp-lock.sh takes a flock in $T/locks
export BACKPLANE_LOCK_URL=http://127.0.0.1:9 BACKPLANE_LOCK_HOME=$T/nohub BACKPLANE_LOCK_DIR=$T/locks; mkdir -p "$T/locks"
WINLOCK=$T/locks/bp-bp-win-gate.lock
# the Windows gate's shutdown check off by default here (no measured time, no margin); the cases below set a VM's measured time in $T/state
export LAND_WIN_STATE=$T/state LAND_WIN_EST_MIN=0 LAND_WIN_MARGIN_MIN=0
mkdir -p "$FAKE"; fails=0
ok() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; fails=$((fails + 1)); fi; }
g() { git -C "$W" "$@"; }
export GIT_AUTHOR_NAME=i2cjak GIT_AUTHOR_EMAIL=build@amemb.com GIT_COMMITTER_NAME=i2cjak GIT_COMMITTER_EMAIL=build@amemb.com
git init -q --bare -b main "$ORIGIN"
W=$T/main; git clone -q "$ORIGIN" "$W" 2>/dev/null; g checkout -q -b main 2>/dev/null
mkdir -p "$W/scripts"; cp "$ROOT/scripts/land.sh" "$ROOT/scripts/land-windows.sh" "$ROOT/scripts/bp-lock.sh" "$W/scripts/"; printf '#!/bin/sh\nexit 0\n' > "$W/scripts/check-author.sh"; chmod +x "$W/scripts/"*.sh
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
grep -q "Windows gate skipped: nothing the hub's Windows tests build from or read changed" "$T/o3" && [ ! -s "$WINS" ]; ok $? "no host change: the Windows gate is skipped, and said"
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
# 16. a host change: a failing Windows gate pushes nothing
mkdir -p "$LANE/native/host"; echo a > "$LANE/native/host/x.rs"; g add -A; g commit -qm "host: a"
before=$(wc -l < "$GATES"); LAND_WIN_TEST="echo win >> $WINS; exit 1" land > "$T/o16"; rc=$?
[ $rc = 1 ] && grep -q "the Windows gate did not pass" "$T/o16" && ! pushed && [ "$(wc -l < "$WINS")" = 1 ] && [ "$(wc -l < "$GATES")" = $((before + 1)) ]; ok $? "the Windows gate fails: nothing pushed, after the Linux gate (rc $rc)"
# 17. it passes: run under the bp-win-gate lock, which is released after; the tree is recorded and the time said
LAND_WIN_TEST="if flock -n $WINLOCK true; then echo free >> $T/lockseen; else echo held >> $T/lockseen; fi; echo win >> $WINS" land > "$T/o17"; rc=$?
[ $rc = 3 ] && [ "$(cat "$T/lockseen")" = held ] && flock -n "$WINLOCK" true && pushed && grep -q "Windows gate passed: [0-9]*m[0-9]*s" "$T/o17" && [ "$(wc -l < "$WINS")" = 2 ]; ok $? "the Windows gate passes under bp-win-gate, the lock released after, its time said, then pushed (rc $rc)"
# 18. the same tree is not tested on Windows twice
land > "$T/o18"; rc=$?; [ $rc = 3 ] && [ "$(wc -l < "$WINS")" = 2 ] && grep -q "Windows tests already passed on this tree" "$T/o18"; ok $? "a passing tree is not tested on Windows again (rc $rc)"
# 19. neither VM up: held with one line naming each VM and why, exit 4, nothing pushed (the real land-windows.sh, stand-ins for ssh and az)
printf '#!/bin/sh\nexit 255\n' > "$T/ssh"; printf '#!/bin/sh\ncase "$*" in *get-instance-view*) echo PowerState/deallocated;; *) echo "az: start refused" >&2; exit 1;; esac\n' > "$T/az"; chmod +x "$T/ssh" "$T/az"
echo b > "$LANE/native/host/x.rs"; g commit -qam "host: b"
( unset LAND_WIN_TEST; LAND_WIN_SSH=$T/ssh LAND_WIN_AZ=$T/az land ) > "$T/o19"; rc=$?
[ $rc = 4 ] && grep -q "^land: HELD: the Windows gate could not run, so nothing is pushed or merged. Tried bp-win: deallocated, and az vm start failed; bp-winbuild: deallocated, and az vm start failed." "$T/o19" && ! pushed && [ "$(wc -l < "$WINS")" = 2 ]; ok $? "no VM up: held, the line names each VM and why, nothing pushed (rc $rc)"
# 19b. a gate whose measured time would cross both VMs' daily auto-shutdowns is not started: held, each named
mkdir -p "$T/state"; echo 1500 > "$T/state/bp-win.minutes"; echo 1500 > "$T/state/bp-winbuild.minutes"
( unset LAND_WIN_TEST; LAND_WIN_SSH=$T/ssh LAND_WIN_AZ=$T/az land ) > "$T/o19b"; rc=$?; rm -f "$T/state/"*.minutes
[ $rc = 4 ] && grep -q "Tried bp-win: a gate now would cross its 07:00 UTC auto-shutdown (it needs about 1500 min with the margin, [0-9]* min are left); bp-winbuild: a gate now would cross its [0-9][0-9]:00 UTC auto-shutdown" "$T/o19b" && ! pushed; ok $? "a gate that would cross both shutdowns: not started, held, each named (rc $rc)"
# 20. LAND_WINDOWS=skip: no Windows gate, said, and noted in the PR body
LAND_WINDOWS=skip land > "$T/o20"; rc=$?; n=$(cat "$FAKE/pr.lane")
[ $rc = 3 ] && grep -q "LAND_WINDOWS=skip: the Windows gate is NOT run" "$T/o20" && grep -q "Windows gate skipped (LAND_WINDOWS=skip)" "$FAKE/body.$n" && [ "$(wc -l < "$WINS")" = 2 ] && pushed; ok $? "LAND_WINDOWS=skip: no gate, said, noted in PR #$n's body (rc $rc)"
LAND_WINDOWS=yes land > "$T/o20b"; rc=$?; [ $rc = 1 ] && grep -q "only LAND_WINDOWS=skip" "$T/o20b"; ok $? "another LAND_WINDOWS value is refused (rc $rc)"
# 21. the verdict of the real land-windows.sh (a stand-in ssh: the tree goes by git push into $T/vm.git, the run prints nextest's ending):
# a failure listed in scripts/land-windows-known.txt does not hold a landing, another does
cp "$ROOT/scripts/land-windows-known.txt" "$LANE/scripts/"; g add -A; g commit -qm "known list"; git init -q --bare "$T/vm.git"
cat > "$T/vmssh" <<'EOS'
#!/bin/sh
# the stand-in VM: every call logged to $VMLOG as "<host> <what>"; the run logs which owner locks are held while it runs
while [ $# -gt 1 ]; do case "$1" in -o) shift 2;; *) break;; esac; done; h=$1; shift   # the options, then the host
lk() { if flock -n "$BACKPLANE_LOCK_DIR/bp-$1.lock" true 2>/dev/null; then echo free; else echo held; fi; }
case "$*" in *receive-pack*) exec git receive-pack "$VMREPO";; 'exit 0') exit 0;; *EncodedCommand*) echo "$h sweep" >> "$VMLOG"; echo "sweep: stopped 0; C:\bp\gate\tmp removed"; exit 0;; esac
grep -q "nextest run" || exit 0   # preparing: nothing to do
echo "$h run bp-win-ui=$(lk bp-win-ui) winbuild-vm=$(lk winbuild-vm)" >> "$VMLOG"
[ -z "${VMSLEEP:-}" ] || exec sleep "$VMSLEEP"
printf '        PASS [   1.0s] (1/3) backplane-hub::bin/backplane-hub a::ok\n     Summary [  2.0s] 3 tests run: 1 passed, 2 failed\n'
for t in $VMFAIL; do printf '  TRY 2 FAIL [   1.0s] (2/3) backplane-hub::bin/backplane-hub %s\n' "$t"; done
[ -z "$VMFAIL" ]
EOS
chmod +x "$T/vmssh"; export VMREPO=$T/vm.git VMLOG=$T/vmlog VMFAIL=
known1=$(sed 's/#.*//' "$ROOT/scripts/land-windows-known.txt" | awk 'NF && $1 != "flaky" { print $1; exit }')
echo c > "$LANE/native/host/x.rs"; g commit -qam "host: c"
( unset LAND_WIN_TEST; LAND_WIN_SSH=$T/vmssh VMFAIL="$known1" land ) > "$T/o21"; rc=$?
[ $rc = 3 ] && grep -q "known Windows failures" "$T/o21" && grep -q "PASSED on bp-win" "$T/o21" && pushed && [ "$(git --git-dir="$T/vm.git" rev-parse refs/heads/gate)" = "$(g rev-parse HEAD)" ]; ok $? "a known Windows failure does not hold the landing; the tree reached the VM by push (rc $rc)"
echo d > "$LANE/native/host/x.rs"; g commit -qam "host: d"
( unset LAND_WIN_TEST; LAND_WIN_SSH=$T/vmssh VMFAIL="$known1 a::new_fault" land ) > "$T/o21b"; rc=$?
[ $rc = 1 ] && grep -q "not known to:" "$T/o21b" && grep -q "^    a::new_fault" "$T/o21b" && ! pushed; ok $? "a new Windows failure holds it: named, nothing pushed (rc $rc)"
vm() { ( unset LAND_WIN_TEST; LAND_WIN_SSH=$T/vmssh land ) ; }
step() { echo "$1" > "$LANE/native/host/x.rs"; g commit -qam "host: $1"; : > "$VMLOG"; }
# 22. one owner per VM: bp-win only when the gate takes bp-win-ui, held for the whole run and released after; swept before and after
step e; vm > "$T/o22"; rc=$?
[ $rc = 3 ] && [ "$(cat "$VMLOG")" = "bp-win sweep
bp-win run bp-win-ui=held winbuild-vm=free
bp-win sweep" ] && flock -n "$T/locks/bp-bp-win-ui.lock" true && pushed; ok $? "bp-win runs it holding bp-win-ui, released after; swept before and after (rc $rc)"
# 23. bp-win-ui held by the UI tests: straight to bp-winbuild under winbuild-vm; bp-win is not touched
step f; ( flock 9; exec sleep 60 ) 9>"$T/locks/bp-bp-win-ui.lock" & ui=$!; sleep 0.3
vm > "$T/o23"; rc=$?; kill $ui 2>/dev/null; wait $ui 2>/dev/null
[ $rc = 3 ] && ! grep -q '^bp-win ' "$VMLOG" && grep -q '^bp-winbuild run bp-win-ui=held winbuild-vm=held$' "$VMLOG" && grep -q "bp-win: bp-win-ui is held" "$T/o23" && grep -q "PASSED on bp-winbuild" "$T/o23" && pushed; ok $? "bp-win-ui held: bp-winbuild under winbuild-vm, bp-win untouched (rc $rc)"
# 24. bp-win's measured time would cross its shutdown: bp-winbuild runs it
step g; mkdir -p "$T/state"; echo 1500 > "$T/state/bp-win.minutes"; vm > "$T/o24"; rc=$?; rm -f "$T/state/bp-win.minutes"
[ $rc = 3 ] && ! grep -q '^bp-win ' "$VMLOG" && grep -q '^bp-winbuild run' "$VMLOG" && grep -q "bp-win: a gate now would cross its 07:00 UTC auto-shutdown" "$T/o24" && [ -s "$T/state/bp-winbuild.minutes" ]; ok $? "bp-win would cross its 07:00 UTC shutdown: bp-winbuild runs it, its time recorded (rc $rc)"
# 25. a failing run is swept after too
step h; VMFAIL=a::new_fault vm > "$T/o25"; rc=$?
[ $rc = 1 ] && [ "$(grep -c '^bp-win sweep' "$VMLOG")" = 2 ] && ! pushed; ok $? "a failed run: swept before and after, nothing pushed (rc $rc)"
# 26. a run cut off at its bound is swept, and the next VM tried; neither finishing holds the landing
step i; LAND_WIN_RUN_MAX_S=2 VMSLEEP=30 vm > "$T/o26"; rc=$?
[ $rc = 4 ] && [ "$(cat "$VMLOG")" = "bp-win sweep
bp-win run bp-win-ui=held winbuild-vm=free
bp-win sweep
bp-winbuild sweep
bp-winbuild run bp-win-ui=free winbuild-vm=held
bp-winbuild sweep" ] && grep -q "bp-win: the run was cut off after" "$T/o26" && ! pushed; ok $? "cut off at its bound: swept, the next VM tried, held (rc $rc)"
# 27. a gate terminated mid-run (TERM) sweeps the VM on its way out; nothing pushed
step j; ( VMSLEEP=30 vm > "$T/o27" ) & lp=$!
for _ in $(seq 100); do grep -q ' run ' "$VMLOG" && break; sleep 0.1; done
pkill -TERM -f "^sh $LANE/scripts/land-windows.sh --vm bp-win\$"; wait $lp; rc=$?
[ $rc != 0 ] && [ "$(tail -n 1 "$VMLOG")" = "bp-win sweep" ] && [ "$(grep -c sweep "$VMLOG")" = 2 ] && ! pushed && flock -n "$T/locks/bp-bp-win-ui.lock" true; ok $? "terminated mid-run: the VM is swept, bp-win-ui released, nothing pushed (rc $rc)"
[ $fails = 0 ] && echo "land_pr_test: all passed" || { echo "land_pr_test: $fails failed"; exit 1; }
