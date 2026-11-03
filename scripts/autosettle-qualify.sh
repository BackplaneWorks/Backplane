#!/bin/sh
# Run on Boat large: this tree's server/checker exceeds the 8 GiB host.
set -eu
mkdir -p build
export BUN_JSC_forceRAMSize=4294967296
export BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576
. scripts/bendok.sh
python3 scripts/laws-shape.py
python3 scripts/i18n-gen.py --check
bend_check src/core/autosettle.bend
scripts/autosettle-proof.sh --scoped
bend test/native/autosettle_dry_run.bend -o build/autosettle-dry-run.mjs
TEST_FILES='test/autosettle_test.bend test/autosettle_sidebar_test.bend test/tickplan_test.bend test/settle_wording_test.bend test/settle_fresh_work_test.bend' scripts/test.sh
TEST_FILES=test/autosettle_order_test.bend scripts/test.sh
bend test/native/autosettle_bench.bend > build/autosettle-timing.log
cat build/autosettle-timing.log
if grep -q "^FAIL" build/autosettle-timing.log; then exit 1; fi
bend_check src/server/server.bend
scripts/autosettle-proof.sh
echo 'ok autosettle qualification'
