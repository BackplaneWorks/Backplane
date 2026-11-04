#!/bin/sh
set -eu
mkdir -p build/bend-vs-ts
python3 -c "import runpy; runpy.run_path('scripts/local-phase.py')['tools']()"
export PATH="$HOME/.local/backplane-ci/bin:$PATH"
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296
export BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576
python3 scripts/local-phase.py --record build/bend-vs-ts/emit.json -- bend src/server/main.bend -o build/bend-vs-ts/hub.js
python3 scripts/local-phase.py --record build/bend-vs-ts/postprocess.json -- bun build --minify-whitespace --minify-syntax --keep-names --target=bun build/bend-vs-ts/hub.js --outfile build/bend-vs-ts/hub-native.js
python3 scripts/js-native.py --server build/bend-vs-ts/hub-native.js > build/bend-vs-ts/native-replacements.txt
python3 test/perf/bend-vs-ts/expose.py build/bend-vs-ts/hub-native.js build/bend-vs-ts/hub-native.mjs
python3 test/perf/bend-vs-ts/expose.py build/bend-vs-ts/hub.js build/bend-vs-ts/hub-raw.mjs
gzip -k build/bend-vs-ts/hub-native.mjs build/bend-vs-ts/hub-raw.mjs
