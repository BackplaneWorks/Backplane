#!/bin/sh
# Called only in a fresh Boat sandbox. The command still performs local guard checks there.
set -eu
python3 -c "import runpy; runpy.run_path('scripts/local-phase.py')['tools']()"
export PATH="$HOME/.local/backplane-ci/bin:$PATH" BACKPLANE_HEAVY_OFFLOADED=1
export BACKPLANE_TELEMETRY=off BACKPLANE_TELEMETRY_BAKE=0 BACKPLANE_GOOGLE_BAKE=0 BACKPLANE_KISTACK_SYNC=0 BACKPLANE_NO_UPDATE=1
exec "$@"
