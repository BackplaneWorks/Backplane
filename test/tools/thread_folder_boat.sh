#!/bin/bash
# Run through boat-job.sh and heavy-boat.sh (3.7 GB, no swap).
set -euo pipefail
mkdir -p build/thread-folder
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824 BUN_JSC_maxPerThreadStackUsage=536870912
. scripts/bendok.sh
python3 scripts/laws-shape.py
python3 test/tools/thread_folder_slices.py
bend_check build/thread-folder/unit/PROOF.bend
bend build/thread-folder/unit/t/test/thread_folder_test.bend -o build/thread-folder/unit.js
# An IO program is run, and FAIL lines cannot count as a pass.
bun build/thread-folder/unit.js | tee build/thread-folder/unit.log
! grep -q '^FAIL' build/thread-folder/unit.log
[ "$(grep -c '^ok ' build/thread-folder/unit.log)" -ge 70 ]
for name in folder rview fallback scratch mailrun files files_state; do
  bend "build/thread-folder/regression-$name/t/test/${name}_test.bend" -o "build/thread-folder/$name.js"
  bun "build/thread-folder/$name.js" | tee "build/thread-folder/$name.log"
  grep -q '^ok ' "build/thread-folder/$name.log"
  ! grep -q '^FAIL' "build/thread-folder/$name.log"
done
bend_check build/thread-folder/proof/PROOF.bend
bend build/thread-folder/proof/PROOF.bend | tee build/thread-folder/proof.log
bend test/files_zip_test.bend -o build/thread-folder/zip.mjs
bun test/tools/files_zip.mjs build/thread-folder/zip.mjs | tee build/thread-folder/zip.log
! grep -q '^FAIL' build/thread-folder/zip.log

bash test/tools/thread_folder_http_boat.sh
