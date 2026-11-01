#!/bin/sh
# Source-only checks for this feature. Never installs or restarts a service.
set -u
cd "$(dirname "$0")/../.."
mkdir -p build/improvements-evidence
out=build/improvements-evidence
export BEND_NO_TELEMETRY=1
export BUN_JSC_forceRAMSize=4294967296
export BUN_JSC_maxPerThreadStackUsage=536870912
export CHECK_JOBS=1 TEST_JOBS=1
ulimit -s 1048576 2>/dev/null || true
status=0
git rev-parse HEAD > "$out/base.txt"
git diff HEAD --binary > "$out/source.diff"
sha256sum "$out/source.diff" > "$out/source.sha256"
run() {
  name=$1
  shift
  timeout 600 "$@" > "$out/$name.log" 2>&1
  rc=$?
  [ "$rc" = 0 ] || status=1
  printf '%s %s\n' "$name" "$rc" | tee -a "$out/gates.txt"
}
: > "$out/gates.txt"
run focused env TEST_FILES='test/improvements_test.bend test/worktree_test.bend' scripts/test.sh
run required-tests scripts/test.sh
run check scripts/check.sh check
# Run the mandated proof even if main's structural/generator preflight failed.
run proof scripts/check.sh proof
# The direct proof run is recorded when check.sh's preflight blocked it.
if ! rg -q 'All terms check|SOME PROOFS FAIL|ALL PROOFS CHECK' "$out/proof.log"; then
  run proof-direct bend PROOF.bend
fi
printf '%s\n' 'Source gates complete; inspect each status and log, no aggregate pass inferred.'
exit "$status"
