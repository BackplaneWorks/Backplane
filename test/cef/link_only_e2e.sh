#!/bin/sh
# link_only_e2e.sh CLIENT_BIN HUB_BIN : a window whose directory holds only a link to libcef.so (as dev builds are laid out) opens the Browser tab and survives.
# Regression: CEF looked for icudtl.dat beside the link, failed a CHECK in cef_initialize and killed the whole app. Needs a display (DISPLAY) and the CEF runtime.
set -eu
CLIENT=$(realpath "$1"); HUB=$(realpath "$2"); RT=${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}; HPORT=${HPORT:-3812}
O=$(mktemp -d "${TMPDIR:-/tmp}/bp-linkonly.XXXXXX"); mkdir -p "$O/home" "$O/proj" "$O/bin" "$O/fh/.cache" "$O/win"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$O/bin/$n"; chmod +x "$O/bin/$n"; done
cp "$CLIENT" "$O/win/backplane-gpui"; ln -s "$RT/libcef.so" "$O/win/libcef.so"
python3 - "$O" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"c","root":R+"/proj","at":N},{"$":"ThreadCreated","id":"t1","project":"p1","title":"b","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
( cd "$O/proj" && BACKPLANE_AGENTS=$O/bin BACKPLANE_NO_UPDATE=1 HOME=$O/fh XDG_CACHE_HOME=$O/fh/.cache exec "$HUB" --home "$O/home" --port $HPORT --host 127.0.0.1 --no-tailscale --foreground > "$O/hub.log" 2>&1 ) & HP=$!
XP=; if [ -z "${DISPLAY:-}" ]; then DISPLAY=:79; export DISPLAY; Xvfb :79 -screen 0 1280x800x24 -nolisten tcp > "$O/xvfb.log" 2>&1 & XP=$!; sleep 2; fi
trap 'kill $HP $XP 2>/dev/null || true; rm -rf "$O"' EXIT INT TERM
n=0; until curl -s -m1 -o /dev/null http://127.0.0.1:$HPORT/hello; do sleep 0.2; n=$((n+1)); [ $n -gt 100 ] && { echo "FAIL hub did not start"; exit 1; }; done
echo '[{"select":0},{"wait":1500},{"click":"rail:Browser"},{"wait":6000}]' > "$O/script.json"
ICD=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1)   # lavapipe on an Xvfb (no DRI3 for a hardware driver)
rc=0; env ${ICD:+VK_ICD_FILENAMES=$ICD} BACKPLANE_GPUI_ASSETS=${E2E_ASSETS:-$(cd "$(dirname "$0")/../.." && pwd)/assets} BACKPLANE_CEF=$RT BACKPLANE_CEF_NO_GPU=1 BACKPLANE_CEF_PROFILE=$O/profile timeout 90 "$O/win/backplane-gpui" --url http://127.0.0.1:$HPORT --token-file "$O/home/token" --script "$O/script.json" --exit-after 20 > "$O/client.log" 2>&1 || rc=$?
[ $rc = 0 ] || { echo "FAIL the window died opening the Browser tab (exit $rc)"; tail -5 "$O/client.log"; exit 1; }
grep -q '"cef_created"' "$O/client.log" && echo "PASS Browser tab opened and the window survived" || { echo "FAIL no browser was created"; grep -v 'GPUI_METRIC {"event":"frame\|asset_missing' "$O/client.log" | tail -15; exit 1; }
