#!/bin/sh
# Regenerate scripts/test-timings.txt: run every test once, serially,
# record each test's wall time, and write the timings longest first.
# scripts/test.sh reads the file to shard tests in CI; a test missing
# from it still runs (scripts/test-shard.py assigns it a shard), so
# this only needs re-running when durations drift badly.
set -eu
cd "$(dirname "$0")/.."
out=$(mktemp scripts/test-timings.XXXXXX)
trap 'rm -f "$out" "$out.profile"' 0 1 2 15
# A caller's focused/fast/sharded selection must never overwrite the full
# duration profile. Production guard and serial admission still apply.
unset TEST_FILES TEST_SHARD TEST_SHARD_COUNT
TEST_FULL=1 TEST_JOBS=1 TEST_TIMINGS_OUT="$out" scripts/test.sh
{ echo "# Recorded by scripts/record-test-timings.sh (seconds, longest first)."
  echo "# Regenerate with: scripts/record-test-timings.sh"
  cat "$out"
} > "$out.profile"
mv "$out.profile" scripts/test-timings.txt
echo "wrote scripts/test-timings.txt"
