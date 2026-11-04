#!/bin/sh
set -eu
mkdir -p build/bend-vs-ts/validation
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576
if [ "${1:-}" != proof ]; then scripts/check.sh check; fi
python3 scripts/local-phase.py --record build/bend-vs-ts/validation/proof.json -- bend PROOF.bend >build/bend-vs-ts/validation/proof.stdout 2>&1 || rc=$?
. scripts/bendok.sh
cat build/bend-vs-ts/validation/proof.stdout
bend_verdict "${rc:-0}" "$(cat build/bend-vs-ts/validation/proof.stdout)"
