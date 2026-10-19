#!/bin/sh
set -eu
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912
bend guide effects > build/telemetry-effects-guide.txt
fail=0
. scripts/bendok.sh
for source in src/core/telemetry.bend src/core/client.bend src/server/telemetry.bend src/server/server.bend src/app/main.bend src/web/view.bend src/mobile/screen.bend test/telemetry_test.bend; do
  rc=0
  bend "$source" --check-only > build/telemetry-one.log 2>&1 || rc=$?
  printf '%s: ' "$source"
  if bend_verdict "$rc" "$(cat build/telemetry-one.log)"; then echo PASS; else cat build/telemetry-one.log; fail=1; fi
done
[ "$fail" = 0 ] || exit 1
bend test/telemetry_test.bend > build/telemetry-one.log 2>&1
cat build/telemetry-one.log
if grep -E '^FAIL|^Error:' build/telemetry-one.log; then exit 1; fi
