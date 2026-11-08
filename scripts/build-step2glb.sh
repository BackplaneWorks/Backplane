#!/bin/sh
# Build dist/backplane-step2glb: the STEP -> GLB helper (tools/step2glb), one
# self-contained executable with the OpenCascade WASM embedded.
# Needs bun (https://bun.sh). BUN_TARGET cross-compiles, e.g. bun-linux-arm64,
# bun-darwin-arm64, bun-windows-x64.
set -eu
cd "$(dirname "$0")/.."
BUN=${BUN:-$(command -v bun || echo "$HOME/.bun/bin/bun")}
. scripts/bun-pin.sh; bun_pin_check "$BUN"
(cd tools/step2glb && "$BUN" install --frozen-lockfile)
mkdir -p dist
"$BUN" build tools/step2glb/step2glb.ts --compile --minify \
  ${BUN_TARGET:+--target="$BUN_TARGET"} --outfile dist/backplane-step2glb
# LGPL-2.1 notices for the embedded OpenCascade / occt-import-js; ship them with the binary.
mkdir -p dist/licenses/step2glb
cp tools/step2glb/node_modules/occt-import-js/dist/license.occt.txt \
   tools/step2glb/node_modules/occt-import-js/dist/license.occt-import-js.txt dist/licenses/step2glb/
ls -la dist/backplane-step2glb
