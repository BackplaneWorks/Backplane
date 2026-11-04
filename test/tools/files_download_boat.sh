#!/bin/bash
# Run only through scripts/boat-job.sh -- scripts/heavy-boat.sh ... .
set -eu
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912 BUN_JSC_forceRAMSize=1073741824 BEND_NO_TELEMETRY=1
mkdir -p build/files-download
python3 - <<'PY'
import json, pathlib
p=pathlib.Path('/sys/fs/cgroup')/pathlib.Path('/proc/self/cgroup').read_text().strip().split('::')[-1].lstrip('/')
assert 0 < int((p/'memory.max').read_text()) <= 3700000000
assert (p/'memory.swap.max').read_text().strip() == '0'
pathlib.Path('build/files-download/cgroup.json').write_text(json.dumps({'memory_max':(p/'memory.max').read_text().strip(),'swap_max':(p/'memory.swap.max').read_text().strip()}))
PY
. scripts/bendok.sh
failed=0
run() {
  local tag=$1; shift
  echo "=== $tag ==="
  local rc=0
  timeout 180 "$@" >"build/files-download/$tag.txt" 2>&1 || rc=$?
  cat "build/files-download/$tag.txt"
  echo "$tag exit $rc"
  if [ "$rc" != 0 ]; then failed=1; fi
}
python3 scripts/laws-shape.py
python3 scripts/i18n-gen.py --check
bend test/files_zip_test.bend -o build/files-download/files_zip.mjs
run zip-bun bun test/tools/files_zip.mjs "$PWD/build/files-download/files_zip.mjs"
run zip-node node test/tools/files_zip.mjs "$PWD/build/files-download/files_zip.mjs"
run desktop node --test desktop/test/downloads.test.cjs desktop/test/no-browser-hub.test.cjs
run source-pin python3 scripts/sidebar-draft-guard.py
run handlers python3 test/tools/files_download_handlers_test.py
run coverage python3 test/tools/files_download_coverage.py
run slices python3 test/tools/files_download_slices.py
for part in route files-ui proof; do
  run "$part-check" bash -c '. scripts/bendok.sh; bend_check "$1"' check "build/files-download/$part/PROOF.bend"
done
cp test/tools/files_download_fixture.bend.in build/files-download/route/http.bend
run http bun test/tools/files_download_http.ts build/files-download/route/http.bend
exit "$failed"
