#!/bin/sh
set -eu
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912
mkdir -p build/ui-laws
python3 test/tools/ui_laws_focus.py
. scripts/bendok.sh
rc=0
bend build/ui-laws/focused.bend > build/ui-laws/focused.log 2>&1 || rc=$?
cat build/ui-laws/focused.log
bend_verdict "$rc" "$(cat build/ui-laws/focused.log)"
bend test/sync_reports_test.bend -o build/ui-laws/sync_reports.mjs > build/ui-laws/sync-emit.log 2>&1
bun test/tools/sync_reports.mjs build/ui-laws/sync_reports.mjs > build/ui-laws/sync-runtime.log 2>&1
cat build/ui-laws/sync-runtime.log
