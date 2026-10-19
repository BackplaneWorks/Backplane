#!/bin/bash
# The pinned Bend compiler emits portable C; Windows compiles it natively.
set -euo pipefail
cd "$(dirname "$0")/.."
export BACKPLANE_LOW_MEM=1 BACKPLANE_JOBS=1 BACKPLANE_EMIT_ONLY=1
export BACKPLANE_GOOGLE_BAKE=0 BP_BUILD_LOCKED=1
if [[ -n ${BACKPLANE_VERSION:-} ]]; then
  export BACKPLANE_VERSION_STAMP=${BACKPLANE_VERSION#v}
fi
mkdir -p build/windows-emitted dist/web
scripts/build-app.sh src/app/main.bend build/backplane-windows.exe
gzip -nc build/cc/backplane-windows.exe/all.c > build/windows-emitted/backplane.c.gz
unset BACKPLANE_VERSION_STAMP
scripts/build-app.sh test/native/windows_runtime.bend build/windows-runtime.exe
gzip -nc build/cc/windows-runtime.exe/all.c > build/windows-emitted/runtime.c.gz
scripts/build-app.sh test/native/windows_providers.bend build/windows-providers.exe
gzip -nc build/cc/windows-providers.exe/all.c > build/windows-emitted/providers.c.gz
bend src/web/index.html -o dist/web
python3 scripts/js-unmarshal.py ../core/client.Ui dist/web/chunk-*.js
python3 scripts/js-native.py dist/web/chunk-*.js
cp src/web/sw.js dist/web/sw.js
for f in dist/web/*.js dist/web/*.css dist/web/*.html; do gzip -9knf "$f"; done
(cd build/windows-emitted && sha256sum *.gz > SHA256SUMS)

scripts/build-windows-helpers.sh
python3 scripts/windows-source.py record
