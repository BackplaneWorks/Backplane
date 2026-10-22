#!/bin/sh
# Run every test program. A test prints lines "ok ..." or "FAIL ..." and the
# script fails when any FAIL appears or a test exits non-zero.
#
# Tests now run in parallel (TEST_JOBS at a time), each exactly as
# before: standalone IO tests use `bend TEST`, and fixtures with a matching
# test/tools/NAME.mjs driver are emitted and exercised by that driver. A
# file with both an IO main and a driver runs both. Keep the same strict
# per-test verdict applied to its full output (a test must exit 0,
# print at least one "^ok " line, print no "^Error:" line, and print
# no "^FAIL" line). Outputs are collected per test and replayed in
# the original alphabetical order, so the report is deterministic
# whatever order the tests finish in.
#
# TEST_JOBS: parallel tests (default: nproc/4, at least 1, at most 6,
# each capped with BUN_JSC_forceRAMSize=4294967296 and nice 19; CI
# sets it to the runner's core count).
# TEST_SHARD / TEST_SHARD_COUNT: run only shard TEST_SHARD (1-based)
# of TEST_SHARD_COUNT, assigned by recorded duration, longest first,
# via scripts/test-shard.py and scripts/test-timings.txt. A test with
# no recorded duration still runs, in exactly one shard (the shard
# script verifies the partition covers every test exactly once).
# TEST_ONLY: run exactly these tests (paths "test/X_test.bend", separated
# by spaces or newlines; TEST_ONLY_FILE names a file of them), in the usual
# order, under the same verdicts. Each must be one of the tests above: an
# unknown name, a repeat or an empty list stops the run (exit 2) before any
# test starts. A selection is never recorded as the whole suite passing.
# TEST_TIMINGS_OUT: when set, per-test wall seconds are written there
# as "<seconds> <path>" lines (scripts/record-test-timings.sh uses it
# to regenerate scripts/test-timings.txt from a serial run).
set -eu
cd "$(dirname "$0")/.."
export BEND_NO_TELEMETRY=1
# Use the same bounded checker/runtime stack as check.sh. Tests exercise
# large wire payloads and real render trees, beyond Bun's default stack.
ulimit -s 1048576 2>/dev/null || true
export BUN_JSC_maxPerThreadStackUsage=${BUN_JSC_maxPerThreadStackUsage:-536870912}

cores=$(nproc 2>/dev/null || echo 4)
jobs=${TEST_JOBS:-$(( cores / 4 ))}
if [ "$jobs" -lt 1 ]; then jobs=1; fi
if [ "$jobs" -gt 6 ]; then jobs=6; fi
# and never more than memory holds: a test's bun takes up to about
# TEST_MEM_GB (default 5) whatever the core count, and a CI runner (16 GB,
# 4 cores) that ran 4 at once was shut down mid-shard
mem_gb=${TEST_MEM_GB:-5}
avail_kb=$(awk '/^MemAvailable:/{print $2}' /proc/meminfo 2>/dev/null || true)
if [ -n "$avail_kb" ]; then
  fit=$(( avail_kb / (mem_gb * 1048576) ))
  [ "$fit" -lt 1 ] && fit=1
  if [ "$jobs" -gt "$fit" ]; then
    echo "test.sh: $jobs jobs asked, $fit fit in $(( avail_kb / 1048576 )) GB available; running $fit" >&2
    jobs=$fit
  fi
fi

all_tests=""
for t in test/*_test.bend; do
  # Deliberately red control used by mobile_cache_identity_gate.py, which
  # verifies its exact failure. It is not a passing regression program.
  [ "$t" = test/cache_identity_before_test.bend ] && continue
  all_tests="$all_tests $t"
done

tests=$all_tests
selected=0
if [ -n "${TEST_ONLY_FILE:-}" ]; then
  [ -r "$TEST_ONLY_FILE" ] || { echo "test.sh: TEST_ONLY_FILE $TEST_ONLY_FILE cannot be read" >&2; exit 2; }
  TEST_ONLY="${TEST_ONLY:-} $(cat "$TEST_ONLY_FILE")"
fi
if [ -n "${TEST_ONLY+x}" ] || [ -n "${TEST_ONLY_FILE:-}" ]; then
  selected=1
  want=$(printf '%s\n' $TEST_ONLY)
  [ -n "$want" ] || { echo "test.sh: TEST_ONLY lists no test" >&2; exit 2; }
  dup=$(printf '%s\n' "$want" | sort | uniq -d)
  [ -z "$dup" ] || { echo "test.sh: TEST_ONLY lists a test twice: $dup" >&2; exit 2; }
  for w in $want; do
    case " $all_tests " in *" $w "*) ;; *) echo "test.sh: TEST_ONLY names $w, which is not one of the tests" >&2; exit 2 ;; esac
  done
  # kept in the suite's own order
  tests=""
  for t in $all_tests; do
    case "
$want
" in *"
$t
"*) tests="$tests $t" ;; esac
  done
  all_tests=$tests
fi
if [ "${TEST_SHARD_COUNT:-1}" -gt 1 ]; then
  tests=$(python3 scripts/test-shard.py scripts/test-timings.txt "${TEST_SHARD:?TEST_SHARD_COUNT is set but TEST_SHARD is not}" "$TEST_SHARD_COUNT" $all_tests)
  tests=" $tests"
fi

# milliseconds since the epoch; macOS's date has no %N, so perl there
now_ms() {
  case $(date +%N) in
    *[!0-9]*|"") perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000' ;;
    *) echo $(( $(date +%s%N) / 1000000 )) ;;
  esac
}

# the inputs' key, taken before any test runs (scripts/verified.sh)
vkey=$(scripts/verified.sh key test 2>/dev/null || true)
work=$(mktemp -d .bend-test.XXXXXX)
trap 'rm -rf "$work"' 0 1 2 15

# Run the selected tests $jobs at a time, one output file each, with
# a sliding window: waiting in whole batches would let one slow test
# idle the other slots, so as soon as $jobs are running each new test
# waits only for the oldest running one. The pid queue is "$@".
set --
i=0
for t in $tests; do
  if [ $# -ge $jobs ]; then
    wait $1 || true
    shift
  fi
  ( s=$(now_ms)
    rc=0
    driver="test/tools/$(basename "$t" _test.bend).mjs"
    # each step's peak RSS appended (cost() keeps the largest)
    tm=""
    [ -x /usr/bin/time ] && tm="/usr/bin/time -a -f %M -o $work/$i.rss"
    (
      export BUN_JSC_forceRAMSize=4294967296
      if [ -f "$driver" ]; then
        if grep -q '^def main(' "$t"; then
          $tm nice -n 19 bend "$t" || exit $?
        fi
        $tm nice -n 19 bend "$t" -o "$work/$i.mjs" || exit $?
        $tm nice -n 19 bun "$driver" "$PWD/$work/$i.mjs"
      else
        $tm nice -n 19 bend "$t"
      fi
    ) >"$work/$i.out" 2>&1 || rc=$?
    echo $rc >"$work/$i.rc"
    e=$(now_ms)
    echo $(( e - s )) >"$work/$i.dur" ) &
  set -- "$@" $!
  i=$(( i + 1 ))
done
while [ $# -gt 0 ]; do
  wait $1 || true
  shift
done

if [ -n "${TEST_TIMINGS_OUT:-}" ]; then
  : >"$TEST_TIMINGS_OUT"
  i=0
  for t in $tests; do
    ms=$(cat "$work/$i.dur" 2>/dev/null || true)
    case $ms in ""|*[!0-9]*) echo "test.sh: no duration for $t" >&2; exit 1 ;; esac
    awk -v ms="$ms" -v t="$t" 'BEGIN { printf "%.2f %s\n", ms / 1000, t }' >>"$TEST_TIMINGS_OUT"
    i=$(( i + 1 ))
  done
  sort -rn -o "$TEST_TIMINGS_OUT" "$TEST_TIMINGS_OUT"
fi

# Replay in order with the original per-test validation. Each verdict
# line also says how long the test ran and its peak memory.
cost() {
  ms=$(cat "$work/$1.dur" 2>/dev/null || echo 0)
  kb=$(grep -E '^[0-9]+$' "$work/$1.rss" 2>/dev/null | sort -n | tail -n1)
  awk -v ms="$ms" -v kb="${kb:-0}" 'BEGIN { if (kb > 0) printf " (%.1fs, %.1f GB)", ms / 1000, kb / 1048576; else printf " (%.1fs)", ms / 1000 }'
}
status=0
i=0
for t in $tests; do
  out="$work/$i.out"
  if [ "$(cat "$work/$i.rc")" != 0 ]; then
    echo "$t: exited non-zero$(cost $i)"; cat "$out"; status=1; i=$(( i + 1 )); continue
  fi
  if grep -q '^ok ' "$out" && ! grep -q '^Error:' "$out"; then
    :
  else
    echo "$t: did not run"; head -20 "$out"; status=1; i=$(( i + 1 )); continue
  fi
  if grep -q '^FAIL' "$out"; then
    echo "$t:$(cost $i)"; grep '^FAIL' "$out"; status=1
  else
    echo "$t: ok$(cost $i)"
  fi
  i=$(( i + 1 ))
done
# every test passed, unsharded: recorded for these inputs (scripts/verified.sh)
if [ $status = 0 ] && [ "${TEST_SHARD_COUNT:-1}" -le 1 ] && [ $selected = 0 ]; then scripts/verified.sh mark test "$vkey" || true; fi
exit $status
