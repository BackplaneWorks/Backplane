#!/bin/sh
set -eu
export CHECK_JOBS=1 CHECK_SHARDS=1 TEST_JOBS=1
export BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576 2>/dev/null || true
mkdir -p build/message-images
node --test test/tools/message_images_test.cjs desktop/test/image-actions.test.cjs
. scripts/bendok.sh
if [ "${1:-}" != --build-only ]; then
bend_check src/server/main.bend
bend_check src/web/app.bend
bend_check src/mobile/app.bend
bend test/images_test.bend
rc=0
bend PROOF.bend >build/message-images/proof.log 2>&1 || rc=$?
echo "proof status $rc"
tail -22 build/message-images/proof.log
fi
# Preserve the complete verdict, even when an unrelated main law is open.
scripts/build-js.sh src/server/main.bend build/message-images/backplane-serve
scripts/dev.sh web

mkdir -p build/star-nudge
bend test/fixtures/star-nudge.bend -o build/star-nudge/fixture.mjs >/dev/null
