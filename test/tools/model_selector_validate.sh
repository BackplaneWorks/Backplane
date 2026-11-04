#!/usr/bin/env bash
# Run in a disposable sandbox through scripts/heavy-boat.sh, which verifies
# the kernel memory cap before this script or the compiler can start.
set -euo pipefail
stage=starting
trap 'printf "[selector] terminated during %s\n" "$stage" >&2; exit 143' TERM
# Keep a long silent compiler run visible through the streamed remote session.
# This child belongs to the watched job and is reaped before the script exits.
(while sleep 10; do printf '[selector] validation still running\n'; done) &
heartbeat=$!
trap 'kill "$heartbeat" 2>/dev/null || true; wait "$heartbeat" 2>/dev/null || true' EXIT
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=1073741824
export BUN_JSC_maxPerThreadStackUsage=536870912
export BACKPLANE_ORDER_SERVER=http://127.0.0.1:9
ulimit -s 1048576
out=build/model-selector-validation
mkdir -p "$out"
stage=compile-catalog
printf '[selector] %s\n' "$stage"
cat /proc/self/cgroup
timeout 300 bend test/model_selector_catalog_test.bend -o "$out/catalog.mjs"
stage=compare-catalog
printf '[selector] %s\n' "$stage"
bun test/tools/model_selector_catalog.mjs "$PWD/$out/catalog.mjs" | tee "$out/catalog.out"
for name in modelpick accounts; do
  stage=$name
  printf '[selector] %s\n' "$stage"
  timeout 180 bend "test/${name}_test.bend" >"$out/${name}.out" 2>&1
  cat "$out/${name}.out"
  grep -q '^ok ' "$out/${name}.out"
  if grep -Eq '^(FAIL|Error:)' "$out/${name}.out"; then exit 1; fi
done
