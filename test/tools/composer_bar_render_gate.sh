#!/bin/sh
# Serial rendering/focused validation inside an isolated remote sandbox.
set -eu
ulimit -s 1048576 2>/dev/null || true
export BUN_JSC_maxPerThreadStackUsage=536870912 BUN_JSC_forceRAMSize=4294967296 BEND_NO_TELEMETRY=1
. scripts/bendok.sh
mkdir -p build/composer-bar
if [ "${1:-}" != --render-only ]; then
for f in test/composer_test.bend test/allowance_ui_test.bend test/composer_local_test.bend test/phone_composer_test.bend test/composer_send_hint_test.bend; do
  echo "Testing $f"
  bend "$f"
done
fi
bend test/tools/composer_bar_fixture.bend > build/composer-bar/after.json
bend src/web/index.html -o build/composer-bar/web
