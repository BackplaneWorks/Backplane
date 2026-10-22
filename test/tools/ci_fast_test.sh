#!/bin/bash
# scripts/ci-fast.py's verdicts, records and fallbacks, against a stand-in
# checkout: fake test.sh/check.sh, a fake boat-job.sh (refuses with 75, or
# runs the command here) and a fake bp-admit.sh (waits, then admits, or is
# killed). A refusal or a long admission wait is never a failure; a job
# killed before its verdict is INCOMPLETE; only a test's own FAIL is FAIL;
# passes are recorded only from a job that exited 0, and reused only for
# byte-identical inputs. No Boat, no bend.
set -u
here=$(cd "$(dirname "$0")/../.." && pwd)
T=$(mktemp -d); trap 'rm -rf "$T"' 0
pass=0 failn=0
ok() { if [ "$1" = "$2" ]; then pass=$((pass+1)); [ -z "${CI_FAST_TEST_VERBOSE:-}" ] || echo "ok   $3"; else failn=$((failn+1)); echo "FAIL $3: got '$1' want '$2'"; fi; }
has() { case $1 in *"$2"*) ok 1 1 "$3" ;; *) ok 0 1 "$3"; printf '%s\n' "$1" | sed 's/^/    | /' ;; esac; }

R=$T/repo; B=$T/bin; mkdir -p "$R/scripts" "$R/test/fixtures" "$B"
cp "$here/scripts/ci-fast.py" "$here/scripts/ci-keys.py" "$here/scripts/ci-runner-id.sh" "$R/scripts/"
for f in test-shard.py check-manifest.py bendok.sh i18n-gen.py; do echo "# stand-in" > "$R/scripts/$f"; done
printf 'import sys\nsys.exit(0)\n' > "$R/scripts/i18n-gen.py"
cat > "$R/scripts/test.sh" <<'EOF'
#!/bin/sh
# stand-in: a verdict line per test of TEST_ONLY; FAKE_FAIL_TESTS fail
s=0
for t in $TEST_ONLY; do
  case " ${FAKE_FAIL_TESTS:-} " in *" $t "*) echo "$t: (0.2s, 0.1 GB)"; echo "FAIL $t broke"; s=1 ;; *) echo "$t: ok (0.5s, 0.1 GB)" ;; esac
done
exit $s
EOF
cat > "$R/scripts/check.sh" <<'EOF'
#!/bin/sh
echo "$1: ${CHECK_FILES:-all}"; echo "$1: 0s"; exit ${FAKE_CHECK_RC:-0}
EOF
chmod +x "$R/scripts/"*.sh
printf 'import Base\n' > "$R/test/a_test.bend"; printf 'import Base\nimport ../src/m.bend as M\n' > "$R/test/b_test.bend"
mkdir -p "$R/src"; printf 'import Base\n' > "$R/src/m.bend"; printf 'import Base\nimport ./src/m.bend as M\n' > "$R/PROOF.bend"
printf 'import Base\n' > "$R/LAWS.bend"; echo fx > "$R/test/fixtures/x.txt"
git -C "$R" init -q; git -C "$R" add -A; git -C "$R" -c user.name=t -c user.email=t@t commit -qm init

printf '#!/bin/sh\necho fake bend\n' > "$B/bend"; chmod +x "$B/bend"
cat > "$B/boat-job" <<'EOF'
#!/bin/bash
# stand-in for scripts/boat-job.sh: FAKE_BOAT=refuse exits 75, run runs the command here
echo "boat $*" >> "$FAKE_LOG"
while [ $# -gt 0 ] && [ "$1" != -- ]; do shift; done; shift
case ${FAKE_BOAT:-run} in
  refuse) echo "boat-job: refused: NO start guard: 161 of 200 Boat starts used this day" >&2; exit 75 ;;
  lost) echo "running: x"; "$@" >/dev/null 2>&1; exit 70 ;;
  *) echo "running: $*"; "$@"; exit $? ;;
esac
EOF
cat > "$B/admit" <<'EOF'
#!/bin/bash
# stand-in for bp-admit.sh: waits FAKE_WAIT seconds in line, then admits; FAKE_ADMIT=kill dies
# first. $FAKE_LOG.q counts reservations in its queue at once; the most is kept in $FAKE_LOG.max
echo "admit $*" >> "$FAKE_LOG"
q() { ( flock 9; n=$(( $(cat "$FAKE_LOG.q" 2>/dev/null || echo 0) + $1 )); echo $n > "$FAKE_LOG.q"; m=$(cat "$FAKE_LOG.max" 2>/dev/null || echo 0); [ $n -gt $m ] && echo $n > "$FAKE_LOG.max" ) 9>"$FAKE_LOG.lk"; }
q 1; trap 'q -1' EXIT
while [ $# -gt 0 ] && [ "$1" != -- ]; do shift; done; shift
for i in $(seq "${FAKE_WAIT:-0}"); do echo "bp-admit: waiting (behind-older=0, avail 20 GB)"; sleep 1; done
[ "${FAKE_ADMIT:-}" = kill ] && exit 143
echo "bp-admit: admitted (2 GB, 2 cores; 0 other jobs running; waited ${FAKE_WAIT:-0}s)"
"$@"
EOF
chmod +x "$B/boat-job" "$B/admit"
export PATH=$B:$PATH CI_FAST_BOAT_JOB=$B/boat-job BP_ADMIT=$B/admit FAKE_LOG=$T/calls LOCAL_CI_LOCK=$T/local.lock LOCAL_CI_BOAT_BACKOFF="1 1" \
  BACKPLANE_VERIFIED=$T/cache BOAT_JOB_ROOT=$T/boat LOCAL_CI_SHARD_SECS=0.4
run() { L=$T/logs$1; shift; out=$(cd "$R" && python3 scripts/ci-fast.py --logs "$L" --steps check,proof,tests "$@" 2>&1); rc=$?; }
calls() { wc -l < "$T/calls" 2>/dev/null || echo 0; }

# 1. Boat refuses everything (start guard) and admission takes 3 s: every step runs here, PASS
FAKE_BOAT=refuse FAKE_WAIT=3 run 1
ok $rc 0 "refused + long admission wait: exit 0"
has "$out" "verdict PASS" "refused + long wait: PASS, never FAIL"
has "$(cat "$L/summary.txt")" "tests=ok 2 run (2 ok, 0 fail, 0 no verdict) in 2 shards, 0 cached; 2 here, waited" "tests: ran here, says so"
w=$(sed -n 's/^  check on local: ok, rc 0, waited \([0-9]*\)s.*/\1/p' "$L/summary.txt"); [ "${w:-0}" -ge 3 ]; ok $? 0 "the admission wait is counted as waiting ($w s)"
ok "$(grep -c '^boat ' "$T/calls")" 12 "each job asked Boat first, then twice more after a backoff"
ok "$(grep -c '^admit ' "$T/calls")" 1 "Boat refusing four jobs makes ONE local reservation"
has "$(cat "$L/local-batch/batch.sh")" "scripts/test.sh" "the one local job runs the jobs one after another"
ok "$(ls "$T/cache/test" | wc -l)" 2 "both tests recorded"
ok "$(ls "$T/cache/check" | wc -l)" 4 "every checked file recorded"
ok "$(ls "$T/cache/proof" | wc -l)" 1 "proof recorded"
ok "$(head -1 "$T/cache/runner/local.txt")" ci-runner-v1 "the runner text the job printed is kept"

# 2. the same inputs again, on the same runner: everything reused, nothing runs
c=$(calls); FAKE_BOAT=refuse run 2 --local
ok $rc 0 "warm: exit 0"; ok $(( $(calls) - c )) 0 "warm: no job at all"
has "$(cat "$L/summary.txt")" "tests=ok 0 run (0 ok, 0 fail, 0 no verdict) in 0 shards, 2 cached" "warm: tests cached"
has "$(cat "$L/summary.txt")" "check=ok 0 run, 4 cached" "warm: check cached"
has "$(cat "$L/tests.log")" "test/a_test.bend: cached ok" "warm: logged as cached ok"

# 3. one test's input changes: only it and the files that read it run
echo '# changed' >> "$R/src/m.bend"
c=$(calls); run 3 --local
ok $rc 0 "one module changed: exit 0"
has "$(cat "$L/summary.txt")" "tests=ok 1 run (1 ok, 0 fail, 0 no verdict) in 1 shards, 1 cached" "one module: only its test runs"
has "$(cat "$L/summary.txt")" "check=ok 2 run, 2 cached" "one module: only its dependents' check runs"
has "$(cat "$L/summary.txt")" "proof=ok run" "one module in the proof's closure: the proof runs"

# 4. killed while waiting for admission: INCOMPLETE (exit 3), never FAIL, nothing recorded
echo '# again' >> "$R/src/m.bend"; n0=$(ls "$T/cache/test" | wc -l)
FAKE_ADMIT=kill FAKE_WAIT=1 run 4 --local
ok $rc 3 "killed in the queue: exit 3"
has "$out" "verdict INCOMPLETE" "killed in the queue: INCOMPLETE"
case $out in *"=FAIL"*|*"verdict FAIL"*) ok 0 1 "killed in the queue: no FAIL anywhere" ;; *) ok 1 1 "killed in the queue: no FAIL anywhere" ;; esac
ok "$(ls "$T/cache/test" | wc -l)" "$n0" "killed: nothing recorded"

# 5. Boat loses the exit status twice (70): INCOMPLETE, not FAIL, not PASS
FAKE_BOAT=lost LOCAL_CI_NO_LOCAL=1 run 5
ok $rc 3 "lost status: exit 3"; has "$out" "verdict INCOMPLETE" "lost status: INCOMPLETE"

# 6. a test's own FAIL line: FAIL (exit 1), and it is not recorded
FAKE_FAIL_TESTS=test/b_test.bend run 6
ok $rc 1 "a failing test: exit 1"
has "$out" "tests=FAIL" "a failing test: tests FAIL"; has "$out" "verdict FAIL" "a failing test: FAIL"
ok "$(ls "$T/cache/test" | wc -l)" "$n0" "a failing test: nothing recorded"

# 7. on Boat: runs there, says so, records under Boat's runner
run 7
ok $rc 0 "on Boat: exit 0"; has "$(cat "$L/summary.txt")" "1 on Boat" "on Boat: says so"
ok "$(head -1 "$T/cache/runner/boat-large.txt")" ci-runner-v1 "on Boat: its runner text kept"

# 8. a check that fails its own verdict: FAIL
echo '# third' >> "$R/src/m.bend"; FAKE_CHECK_RC=1 run 8
ok $rc 1 "a failing check: exit 1"; has "$out" "check=FAIL" "a failing check: check FAIL"

# 9. --full reuses nothing and still records
c=$(calls); run 9 --full
ok $rc 0 "full: exit 0"; has "$(cat "$L/summary.txt")" "tests=ok 2 run" "full: every test runs"

# a dry run says what would run and runs nothing
echo '# dry' >> "$R/src/m.bend"; c=$(calls); run 9d --dry-run
ok $rc 0 "dry run: exit 0"; ok $(( $(calls) - c )) 0 "dry run: no job"
has "$out" "tests: 1 to run, 1 reused" "dry run: what would run"; has "$out" "new sandbox" "dry run: where"

# 10. two runs at once, both refused by Boat: never more than one local reservation in the queue
rm -f "$T/calls.max"; echo '# four' >> "$R/src/m.bend"
R2=$T/repo2; git clone -q "$R" "$R2"; cp "$R/src/m.bend" "$R2/src/m.bend"
( cd "$R" && FAKE_BOAT=refuse FAKE_WAIT=2 python3 scripts/ci-fast.py --logs "$T/logs10a" --steps tests >/dev/null 2>&1 ) &
( cd "$R2" && FAKE_BOAT=refuse FAKE_WAIT=2 BACKPLANE_VERIFIED=$T/cache2 python3 scripts/ci-fast.py --logs "$T/logs10b" --steps tests >/dev/null 2>&1 ) &
wait
ok "$(cat "$T/calls.max")" 1 "two refused runs at once: at most one local reservation at a time"
ok "$(cat "$T/logs10a/summary.txt" "$T/logs10b/summary.txt" | grep -c 'verdict PASS')" 2 "two refused runs: both still pass"

echo "ci_fast_test: $pass passed, $failn failed"
[ $failn = 0 ]
