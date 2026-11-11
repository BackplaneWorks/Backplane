#!/bin/bash
# Sync accounts end to end on two temp hubs (sync.py): builds nothing; runs this checkout's debug hub with the stand-in claude.
set -u
WT=$(cd "$(dirname "$0")/../../.." && pwd); . "$WT/scripts/lib-bptmp.sh"
W=$BP_TMP/bp-syncaccounts-e2e; rm -rf "$W"; mkdir -p "$W"
cp "${HUB_BIN:-$WT/native/host/target/debug/backplane-hub}" "$W/hub" || { echo "build the hub first: scripts/cargo-q build --bin backplane-hub"; exit 2; }
timeout 900 python3 -I "$WT/test/e2e/syncaccounts/sync.py" "$W/hub" "$W"; rc=$?
for f in "$W/A.log" "$W/B.log"; do echo "== $(basename "$f")"; grep -i -E 'sync account|accounts:|panic' "$f" | tail -12; done
[ -n "${KEEP:-}" ] || rm -rf "$W"
exit $rc
