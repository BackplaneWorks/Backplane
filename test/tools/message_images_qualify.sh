#!/bin/sh
set -eu
export BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576 2>/dev/null || true
mkdir -p build/message-images build/star-nudge
bend test/sidebar_draft_image_test.bend > build/message-images/sidebar-qualification.log 2>&1
cat build/message-images/sidebar-qualification.log
bend test/fixtures/star-nudge.bend -o build/star-nudge/fixture.mjs >/dev/null
