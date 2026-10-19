#!/bin/sh
set -eu
mkdir -p build
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296 CHECK_JOBS=1 TEST_JOBS=1
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912
. scripts/bendok.sh
# The prior pinned run passed all 47 telemetry checks. The integration
# checkpoint changes hub/build wiring and fixes one onboarding fixture.
bend_check src/app/main.bend || exit 1
for test in test/onboard_test.bend test/cli_test.bend test/mailboot_test.bend; do
  rc=0
  bend "$test" > build/telemetry-one-test.log 2>&1 || rc=$?
  cat build/telemetry-one-test.log
  [ "$rc" = 0 ] || exit "$rc"
  if grep -E '^FAIL|^Error:' build/telemetry-one-test.log; then exit 1; fi
  grep '^ok ' build/telemetry-one-test.log >/dev/null
done
python3 test/tools/telemetry_build_test.py
scripts/check.sh all
