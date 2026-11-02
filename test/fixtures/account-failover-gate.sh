#!/bin/sh
# Serial simulated account-exhaustion gate, run only in the admitted sandbox.
set -eu
mkdir -p build/account-failover
export BUN_JSC_forceRAMSize=4294967296 CHECK_JOBS=1 CHECK_SHARDS=1 CHECK_FALLBACK=0 TEST_JOBS=1
actual=$(sha256sum "$(command -v bend)" | cut -d' ' -f1)
[ "$actual" = b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643 ] || { echo "Unexpected Bend toolchain: $actual"; exit 1; }
printf '%s\n' "$actual" > build/account-failover/toolchain.sha256
TEST_FILES='test/account_failover_test.bend test/fallback_test.bend test/accounts_test.bend test/sidebar_draft_image_test.bend' scripts/test.sh > build/account-failover/focused.log 2>&1
cat build/account-failover/focused.log
