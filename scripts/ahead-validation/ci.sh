#!/bin/sh
# Explicit qualification on an admitted host; check.sh owns its bend lock.
# The current 18 GiB proof floor exceeds Boat's 16 GiB large sandbox.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/ahead-validation
if [ "${1:-}" != --prepared-tools ]; then
  python3 -c "import runpy; runpy.run_path('scripts/local-phase.py')['tools']()"
  export PATH="$HOME/.local/backplane-ci/bin:$PATH"
fi
export BACKPLANE_HEAVY_OFFLOADED=1
ulimit -s 1048576 2>/dev/null || true
scripts/check.sh > build/ahead-validation/check.log 2>&1
python3 -B -m unittest discover -s test/tools -p ahead_validation_test.py
python3 scripts/ahead-validation/prepare.py --cache-dir build/ahead-validation/cache

# Memory qualification is a separate cold lane against a hash-bound service.
# UI preparation/receipt reuse never implies that this runtime gate passed.
if [ -n "${AHEAD_MEMORY_SOAK_BIN:-}" ]; then
  : "${AHEAD_MEMORY_SOAK_SHA256:?set the packaged hub SHA256}"
  scripts/ahead-validation/memory.sh "$AHEAD_MEMORY_SOAK_BIN" "$AHEAD_MEMORY_SOAK_SHA256"
else
  echo "ahead-validation: memory soak NOT RUN (no packaged binary supplied)"
fi
