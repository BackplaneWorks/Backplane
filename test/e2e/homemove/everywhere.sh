#!/bin/bash
# Bub everywhere from its home (everywhere.py) on two temp hubs, ports 3901/3902: HUB_BIN (default: this checkout's debug hub) and the checks' table.
set -u
WT=$(cd "$(dirname "$0")/../../.." && pwd); . "$WT/scripts/lib-bptmp.sh"
BIN=${1:-$WT/native/host/target/debug/backplane-hub}
W=$BP_TMP/bp-everywhere-e2e; rm -rf "$W"; mkdir -p "$W"
cp "$BIN" "$W/hub" || { echo "build the hub first: scripts/cargo-q build --bin backplane-hub"; exit 2; }
timeout 900 python3 -I "$WT/test/e2e/homemove/everywhere.py" "$W/hub" "$W"; rc=$?
for f in "$W/A.log" "$W/B.log"; do echo "== $(basename "$f")"; grep -i -E 'home move|hub offer|panic' "$f" | tail -12; done
pkill -f "$W/hub --home" 2>/dev/null   # a hub that would not stop (it runs in a session of its own)
[ -n "${KEEP:-}" ] || rm -rf "$W"
exit $rc
