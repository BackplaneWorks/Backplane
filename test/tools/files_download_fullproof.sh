#!/bin/bash
# A separate capped attempt: never interpret termination as a proof pass.
set -eu
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912 BUN_JSC_forceRAMSize=1073741824 BEND_NO_TELEMETRY=1
mkdir -p build/files-download-proof
bend PROOF.bend >build/files-download-proof/full.txt 2>&1
cat build/files-download-proof/full.txt
