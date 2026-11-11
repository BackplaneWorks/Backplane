#!/bin/bash
# Sign in there end to end on two temp hubs (there.py): builds nothing; runs this checkout's hub with stand-in CLIs. Ports 3951-3952.
set -u
WT=$(cd "$(dirname "$0")/../../.." && pwd); . "$WT/scripts/lib-bptmp.sh"
W=$BP_TMP/bp-signinthere-e2e; rm -rf "$W"; mkdir -p "$W"
cp "${HUB_BIN:-$WT/native/host/target/debug/backplane-hub}" "$W/hub" || { echo "build the hub first: scripts/cargo-q build --bin backplane-hub"; exit 2; }
timeout 900 python3 -I "$WT/test/e2e/signinthere/there.py" "$W/hub" "$W"; rc=$?
for f in "$W/A.log" "$W/B.log"; do echo "== $(basename "$f")"; grep -i -E 'signin|sign-in|panic' "$f" | tail -12; done
[ -n "${KEEP:-}" ] || rm -rf "$W"
exit $rc
