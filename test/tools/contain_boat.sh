#!/bin/bash
# Run through boat-job.sh + heavy-boat.sh (3.7 GB, no swap): exact declaration closures.
set -euo pipefail
mkdir -p build/contain
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824 BUN_JSC_maxPerThreadStackUsage=536870912
. scripts/bendok.sh
python3 scripts/laws-shape.py
python3 test/tools/contain_slices.py
bun --version
bun test/tools/contain_fs.mjs | tee build/contain/fs.log
! grep -q '^FAIL' build/contain/fs.log
[ "$(grep -c '^ok ' build/contain/fs.log)" -ge 30 ]
for name in zip farview render plot mech conn; do
  bend_check "build/contain/$name/PROOF.bend"
done
bend build/contain/stream/t/test/far_stream_test.bend -o build/contain/stream.js
bun build/contain/stream.js | tee build/contain/stream.log
! grep -q '^FAIL' build/contain/stream.log
grep -q '^ok ' build/contain/stream.log
bend test/files_zip_test.bend -o build/contain/files_zip.mjs
bun test/tools/files_zip.mjs build/contain/files_zip.mjs | tee build/contain/files_zip.log
! grep -q '^FAIL' build/contain/files_zip.log
# the largest closure last (the two view handlers), so the verdicts above stand if it is refused
bend_check build/contain/view/PROOF.bend
