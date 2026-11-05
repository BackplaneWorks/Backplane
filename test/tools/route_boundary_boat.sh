#!/bin/bash
# Run through boat-job.sh (3.7 GB, no swap): exact declaration closures only.
set -euo pipefail
mkdir -p build/route-boundary
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824 BUN_JSC_maxPerThreadStackUsage=536870912
. scripts/bendok.sh
python3 scripts/laws-shape.py
python3 test/tools/route_boundary_slices.py
for name in git review watchpol mcp srvwatch; do
  bend_check "build/route-boundary/$name/PROOF.bend"
done
bend build/route-boundary/unit/t/test/route_boundary_test.bend -o build/route-boundary/unit.js
bun build/route-boundary/unit.js | tee build/route-boundary/unit.log
! grep -q '^FAIL' build/route-boundary/unit.log
[ "$(grep -c '^ok ' build/route-boundary/unit.log)" -ge 54 ]
for name in rview thread_folder; do
  bend "build/route-boundary/regression-$name/t/test/${name}_test.bend" -o "build/route-boundary/$name.js"
  bun "build/route-boundary/$name.js" | tee "build/route-boundary/$name.log"
  grep -q '^ok ' "build/route-boundary/$name.log"
  ! grep -q '^FAIL' "build/route-boundary/$name.log"
done
# The version proof on real temporary repositories (git.bend's Git.ident.sh, as printed).
bend build/route-boundary/ident/t/test/ident_script.bend -o build/route-boundary/ident.js
bun build/route-boundary/ident.js > build/route-boundary/ident.sh
bash test/tools/ident_fs.sh build/route-boundary/ident.sh | tee build/route-boundary/ident-fs.log
! grep -q '^FAIL' build/route-boundary/ident-fs.log
[ "$(grep -c '^ok ' build/route-boundary/ident-fs.log)" -ge 25 ]
