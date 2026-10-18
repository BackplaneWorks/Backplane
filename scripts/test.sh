#!/bin/sh
# Run every test program. A test prints lines "ok ..." or "FAIL ..." and the
# script fails when any FAIL appears or a test exits non-zero.
#
# Tests now run in parallel (TEST_JOBS at a time), each exactly as
# before: the same `bend TEST` invocation, and the same strict
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
# TEST_TIMINGS_OUT: when set, per-test wall seconds are written there
# as "<seconds> <path>" lines (scripts/record-test-timings.sh uses it
# to regenerate scripts/test-timings.txt from a serial run).
set -eu
cd "$(dirname "$0")/.."
export BEND_NO_TELEMETRY=1

cores=$(nproc 2>/dev/null || echo 4)
jobs=${TEST_JOBS:-$(( cores / 4 ))}
if [ "$jobs" -lt 1 ]; then jobs=1; fi
if [ "$jobs" -gt 6 ]; then jobs=6; fi

all_tests=""
for t in test/*_test.bend; do all_tests="$all_tests $t"; done

tests=$all_tests
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
    BUN_JSC_forceRAMSize=4294967296 nice -n 19 bend "$t" >"$work/$i.out" 2>&1 || rc=$?
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

# Replay in order with the original per-test validation.
status=0
i=0
for t in $tests; do
  out="$work/$i.out"
  if [ "$(cat "$work/$i.rc")" != 0 ]; then
    echo "$t: exited non-zero"; cat "$out"; status=1; i=$(( i + 1 )); continue
  fi
  if grep -q '^ok ' "$out" && ! grep -q '^Error:' "$out"; then
    :
  else
    echo "$t: did not run"; head -20 "$out"; status=1; i=$(( i + 1 )); continue
  fi
  if grep -q '^FAIL' "$out"; then
    echo "$t:"; grep '^FAIL' "$out"; status=1
  else
    echo "$t: ok"
  fi
  i=$(( i + 1 ))
done
# every test passed, unsharded: recorded for these inputs (scripts/verified.sh)
if [ $status = 0 ] && [ "${TEST_SHARD_COUNT:-1}" -le 1 ]; then scripts/verified.sh mark test "$vkey" || true; fi
exit $status
