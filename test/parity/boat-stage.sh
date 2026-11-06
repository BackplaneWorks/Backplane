#!/bin/sh
# Stage what the parity run needs on Boat (the manifest of a boat-job with --include-file is only the listed files).
# usage: from the repo root: sh test/parity/boat-stage.sh  -> prints the --include-file arguments
set -eu
mkdir -p stage; cp build/gpui/backplane-serve stage/backplane-serve
{ ls test/parity/*.mjs test/parity/*.sh test/wire.mjs test/perf/lib.mjs test/slim/check_agents.sh test/tools/perf_standin.ts src/web/wire.js src/web/view-delta.js src/core/cbor; echo stage/backplane-serve; } | sed 's/^/--include-file /' | tr '\n' ' '
