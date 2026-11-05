#!/bin/sh
# Boat only, fixtures only: /message-image on the Rust hub vs the JS reference hub. Stage first (from the repo root):
#   mkdir -p stage-mimg && cp build/gpui/files-bin/backplane-hub stage-mimg/hub && cp build/gpui/backplane-serve-960 stage-mimg/js
# then:   sh test/gpui/files/boat-mimg-job.sh      (output: out/mimg/result.json, differences printed)
INC=""; for f in test/gpui/files/e2e_mimg.mjs test/gpui/perf/lib.mjs test/gpui/wire.mjs src/web/wire.js src/core/cbor.bend test/tools/perf_standin.ts stage-mimg/hub stage-mimg/js; do INC="$INC --include-file $f"; done
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/mimg $INC -- sh -c '
mkdir -p out/mimg; export PATH="$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
chmod +x stage-mimg/hub stage-mimg/js; PARITY_WORK=/tmp/pw bun test/gpui/files/e2e_mimg.mjs $PWD/stage-mimg/hub $PWD/stage-mimg/js out/mimg/result.json > out/mimg/run.txt 2>&1; m=$?; tail -30 out/mimg/run.txt; exit $m'
