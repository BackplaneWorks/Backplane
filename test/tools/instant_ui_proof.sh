#!/bin/sh
# work_run owns the bend lock for this command. Do not call a second lock
# guard here: its FD is not inherited from Backplane's native lock holder.
set -eu
mkdir -p build/instant-ui
python3 - <<'PY'
import runpy
m=runpy.run_path('scripts/heavy-guard.py')
if not m['admit']('proof'): raise SystemExit(75)
PY
ulimit -s 1048576
export BEND_NO_TELEMETRY=1 BUN_JSC_maxPerThreadStackUsage=536870912 BUN_JSC_forceRAMSize=4294967296
. scripts/bendok.sh
rc=0
bend PROOF.bend > build/instant-ui/PROOF.log 2>&1 || rc=$?
cat build/instant-ui/PROOF.log
bend_verdict "$rc" "$(cat build/instant-ui/PROOF.log)"
