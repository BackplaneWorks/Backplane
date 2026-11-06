#!/bin/sh
# open_e2e.sh HUB_BIN : a thread with no browser page: /bots/<id>/screen.jpg is 404; the Browser tab's `bots.browser open` makes the hub's helper open one and the
# frame appears (the fix for the tab saying "no browser frame yet" forever). Hub only, no display. usage: test/cef/open_e2e.sh HUB_BIN
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd); HUB=$1; OUT=${OUT:-/tmp/cef-open-e2e}; HPORT=${HPORT:-3879}
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fakehome/.cache"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/$n"; chmod +x "$OUT/bin/$n"; done
printf '#!/bin/sh\ncd %s/tools/browser && exec %s src/main.ts --engine chrome "$@" 2>> %s/helper.err\n' "$ROOT" "$(command -v bun)" "$OUT" > "$OUT/bin/backplane-browser"; chmod +x "$OUT/bin/backplane-browser"
python3 - "$OUT" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"o","root":R+"/proj","at":NOW},{"$":"ThreadCreated","id":"t1","project":"p1","title":"b","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
( cd "$OUT/proj" && BACKPLANE_AGENTS=$OUT/bin BACKPLANE_BROWSER=$OUT/bin/backplane-browser BACKPLANE_NO_UPDATE=1 HOME=$OUT/fakehome XDG_CACHE_HOME=$OUT/fakehome/.cache exec "$HUB" --home "$OUT/home" --port $HPORT --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.log" 2>&1 ) & HP=$!
trap 'kill $HP 2>/dev/null || true' EXIT INT TERM
n=0; until curl -s -m1 -o /dev/null http://127.0.0.1:$HPORT/hello; do sleep 0.2; n=$((n+1)); [ $n -gt 100 ] && { echo "hub did not start"; exit 1; }; done
T=$(cat "$OUT/home/token"); code() { curl -s -m5 -o "$OUT/s.jpg" -w '%{http_code}' -H "Authorization: Bearer $T" http://127.0.0.1:$HPORT/bots/t1/screen.jpg; }
C0=$(code); echo "before open: $C0"; [ "$C0" = 404 ] || { echo "FAIL expected 404 before open"; exit 1; }
bun "$HERE/hub_rpc.mjs" $HPORT "$OUT/home/token" '[{"m":"bots.browser","p":{"bot":"t1","op":"open"},"wait":3000}]'
n=0; C=000; while [ $n -lt 40 ]; do C=$(code); [ "$C" = 200 ] && break; sleep 0.5; n=$((n+1)); done
echo "after open: $C ($(wc -c < "$OUT/s.jpg") bytes)"; [ "$C" = 200 ] && echo "PASS the Browser tab's page appears after open" || { echo "FAIL no frame after open"; tail -5 "$OUT/helper.err"; exit 1; }
