#!/bin/bash
# The home move end to end on two temp hubs (move.py): builds the hub of this checkout, runs it, prints the checks and the hubs' move lines.
set -u
WT=$(cd "$(dirname "$0")/../../.." && pwd); . "$WT/scripts/lib-bptmp.sh"
W=$BP_TMP/bp-homemove-e2e; rm -rf "$W"; mkdir -p "$W"
cp "$WT/native/host/target/debug/backplane-hub" "$W/hub" || { echo "build the hub first: scripts/cargo-q build --bin backplane-hub"; exit 2; }
timeout 900 python3 -I "$WT/test/e2e/homemove/move.py" "$W/hub" "$W"; rc=$?
for f in "$W/A.log" "$W/B.log"; do echo "== $(basename "$f")"; grep -i -E 'home move|hub offer|panic' "$f" | tail -12; done
[ -n "${KEEP:-}" ] || rm -rf "$W"
exit $rc
