#!/bin/sh
# Focused qualification after normal adoption of current main; no app build.
set -eu
export BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576 2>/dev/null || true
mkdir -p build/message-images-integration
bend test/sidebar_draft_image_test.bend >build/message-images-integration/sidebar.log 2>&1
cat build/message-images-integration/sidebar.log
