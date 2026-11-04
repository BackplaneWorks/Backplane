#!/usr/bin/env bash
# Supporting integration evidence; requires the kernel cap in heavy-boat.sh.
set -euo pipefail
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824
export BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576
python3 test/tools/model_selector_focus.py
out=build/model-selector-focus
printf '[selector] compile focused render\n'
timeout 240 bend "$out/t/test/model_selector_render_test.bend" -o "$out/render.mjs"
bun test/tools/model_selector_render.mjs "$PWD/$out/render.mjs" | tee "$out/render.out"
printf '[selector] check focused proof\n'
. scripts/bendok.sh
bend_check "$out/PROOF.bend"
printf 'ok focused UI and added-law checks; full application/proof gate not run\n'
