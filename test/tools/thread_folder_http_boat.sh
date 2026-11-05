#!/bin/bash
set -euo pipefail
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824 BUN_JSC_maxPerThreadStackUsage=536870912
mkdir -p build/thread-folder
python3 test/tools/thread_folder_http_slices.py
modes=fixed
[ "${THREAD_FOLDER_BEFORE:-0}" != 1 ] || modes="before fixed"
for mode in $modes; do
  bend "build/thread-folder/$mode/http.bend" -o "build/thread-folder/$mode/http.js"
  bun test/tools/thread_folder_http.ts "build/thread-folder/$mode/http.js" "$mode" > "build/thread-folder/$mode/matrix.json"
  cat "build/thread-folder/$mode/matrix.json"
done
