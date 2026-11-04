#!/bin/bash
set -eu
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912 BUN_JSC_forceRAMSize=1073741824 BEND_NO_TELEMETRY=1
mkdir -p build/files-download
# This program is invoked only in the kernel-capped Boat executor.
python3 test/tools/files_download_regressions.py
