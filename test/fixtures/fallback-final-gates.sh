#!/bin/sh
set -eu
# Full integration gate after rebase and native-layout/server factoring.
export BUN_JSC_forceRAMSize=4294967296 CHECK_JOBS=1 TEST_JOBS=2
. scripts/bendok.sh
bend_check src/server/server.bend
bend_check src/app/layout.bend
bend test/fallback_test.bend
scripts/check.sh all
scripts/test.sh
