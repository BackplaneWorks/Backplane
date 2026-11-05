#!/bin/bash
# Run through boat-job.sh (3.7 GB, no swap): exact declaration closures only.
set -euo pipefail
mkdir -p build/route-boundary
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824 BUN_JSC_maxPerThreadStackUsage=536870912
. scripts/bendok.sh
python3 scripts/laws-shape.py
python3 test/tools/route_boundary_slices.py
for name in git review; do
  bend_check "build/route-boundary/$name/PROOF.bend"
done
bend build/route-boundary/unit/t/test/route_boundary_test.bend -o build/route-boundary/unit.js
bun build/route-boundary/unit.js | tee build/route-boundary/unit.log
! grep -q '^FAIL' build/route-boundary/unit.log
[ "$(grep -c '^ok ' build/route-boundary/unit.log)" -ge 25 ]
for name in rview thread_folder; do
  bend "build/route-boundary/regression-$name/t/test/${name}_test.bend" -o "build/route-boundary/$name.js"
  bun "build/route-boundary/$name.js" | tee "build/route-boundary/$name.log"
  grep -q '^ok ' "build/route-boundary/$name.log"
  ! grep -q '^FAIL' "build/route-boundary/$name.log"
done
