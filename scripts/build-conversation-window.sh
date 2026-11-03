#!/bin/sh
set -eu
mkdir -p build/bounded-view
ulimit -s 1048576 2>/dev/null || true
export BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912 BEND_NO_TELEMETRY=1
scripts/build-js.sh src/server/main.bend build/bounded-view/backplane-serve
rm -rf build/bounded-view/web
bend src/web/index.html -o build/bounded-view/web
python3 scripts/js-unmarshal.py ../core/client.Ui build/bounded-view/web/chunk-*.js
python3 scripts/js-native.py build/bounded-view/web/chunk-*.js
cp src/web/sw.js build/bounded-view/web/sw.js
sh scripts/build-viewer.sh build/bounded-view/web/viewer/vendor
