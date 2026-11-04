#!/bin/sh
# Called only in a fresh Boat sandbox. The command still performs local guard checks there.
# The whole command runs in a kernel-enforced scope (memory.max 3.7e9 B, swap 0): the installed bend/bun guard
# (scripts/compiler-guard.py) refuses to start a compiler on Boat without a kernel-read cap. No scope available: the
# command runs unscoped and a guarded bend/bun refuses (fail closed).
set -eu
python3 -c "import runpy; runpy.run_path('scripts/local-phase.py')['tools']()"
export PATH="$HOME/.local/backplane-ci/bin:$PATH" BACKPLANE_HEAVY_OFFLOADED=1
export BACKPLANE_TELEMETRY=off BACKPLANE_TELEMETRY_BAKE=0 BACKPLANE_GOOGLE_BAKE=0 BACKPLANE_KISTACK_SYNC=0 BACKPLANE_NO_UPDATE=1
if [ -z "${BACKPLANE_HEAVY_SCOPED:-}" ] && command -v systemd-run >/dev/null 2>&1; then
  BACKPLANE_HEAVY_SCOPED=1 exec systemd-run --user --scope -q -p MemoryAccounting=yes -p MemoryMax=3700000000 -p MemorySwapMax=0 -- "$@"
fi
exec "$@"
