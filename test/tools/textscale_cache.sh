#!/bin/sh
# Separate stages distinguish a checker/emit cap failure from a failed test.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/text-scaling
. scripts/bendok.sh
export BEND_NO_TELEMETRY=1
export BUN_JSC_forceRAMSize=${BUN_JSC_forceRAMSize:-8589934592}
printf 'TYPECHECK start\n'
bend_check test/textscale_test.bend
printf 'TYPECHECK pass; JS emit start\n'
bend test/textscale_test.bend -o build/text-scaling/textscale-test.js
printf 'JS emit pass; assertions start\n'
bun build/text-scaling/textscale-test.js > build/text-scaling/cache-results.log
cat build/text-scaling/cache-results.log
if grep -q '^FAIL' build/text-scaling/cache-results.log; then exit 1; fi
printf 'Cache/semantics regression PASS\n'
