#!/bin/sh
# Boat: the dropped-link proof against the Rust hub (stand-in agents, 20 streaming), hard cap per hub inside reconnect_prove.mjs.
# usage: from the repo root, with the hub binary at stage/backplane-hub (untracked): sh test/gpui/wire/boat-prove.sh
set -eu
[ -x stage/backplane-hub ] || { echo "stage/backplane-hub missing"; exit 2; }
exec scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/prove \
  --include-file stage/backplane-hub $(ls test/gpui/wire/reconnect_prove.mjs test/gpui/wire.mjs test/gpui/perf/lib.mjs test/tools/perf_standin.ts src/web/wire.js src/core/cbor.bend 2>/dev/null | sed 's/^/--include-file /' | tr '\n' ' ') \
  -- sh -c 'which bun >/dev/null 2>&1 || export PATH="$HOME/.bun/bin:$PATH"; which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }; mkdir -p out/prove; chmod +x stage/backplane-hub; BP_ROOT=$PWD bun test/gpui/wire/reconnect_prove.mjs $PWD/stage/backplane-hub 3875 20 out/prove/rust.json > out/prove/rust.txt 2>&1; rc=$?; echo rc=$rc >> out/prove/rust.txt; grep -E "PASS|FAIL|RESULT|rc=|Error" out/prove/rust.txt; exit $rc'
