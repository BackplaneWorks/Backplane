#!/bin/bash
# Edit on a person's real desktop session (KEMBED_DISPLAY, e.g. :0) with a COPY of a project (KEMBED_PROJ_SRC): an isolated hub and client
# of their own, no XTest input (the person's mouse and keyboard are never driven), only the client's own window captured.
#   KEMBED_DISPLAY=:0 KEMBED_PROJ_SRC=~/Documents/x/Board EMBED_MODE=fork test/kicad_embed/desktop_smoke.sh
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
: "${KEMBED_DISPLAY:?set KEMBED_DISPLAY to the desktop display}" "${KEMBED_PROJ_SRC:?set KEMBED_PROJ_SRC to the project to copy}"
discover_bins
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
DISP="$KEMBED_DISPLAY"; export DISPLAY="$KEMBED_DISPLAY" GDK_BACKEND=x11 GDK_SCALE=1
start_hub
cat >"$OUT/script.json" <<'J'
[{"select":0},{"wait":2500},{"click":"rail:Board"},{"wait":3000},{"click":"ui:Edit"},{"wait":15000},{"mark":"shown"},{"kicad":{"report":true}},{"wait":2000},
 {"click":"rail:Schematic"},{"wait":5000},{"mark":"sch"},{"kicad":{"report":true}},{"wait":2000},{"click":"ui:View"},{"wait":2000}]
J
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 60 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
# the client's own window (by its pid), captured at each mark
win() { for w in $(xwininfo -root -tree | awk '/Backplane/ {print $1}'); do [ "$(xprop -id "$w" _NET_WM_PID 2>/dev/null | awk '{print $3}')" = "$CP" ] && { echo "$w"; return; }; done; }
for m in shown sch; do
  for _ in $(seq 300); do grep -q "\"name\":\"$m\"" "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
  sleep 1.5; w=$(win); [ -n "$w" ] && import -window "$w" "$OUT/shots/$m.png" 2>/dev/null || true
done
wait "$CP" 2>/dev/null || true
grep -h '"event":"kicad"' "$OUT/client.log" | python3 -c '
import json,sys
for l in sys.stdin:
    r=json.loads(l[l.index("{"):]); sh=[f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")]
    print(r.get("tag") or "", "state=",r.get("state"),"mode=",r.get("mode"),"reason=",r.get("reason"),"shown=",[(f.get("kind"),f.get("w"),f.get("h"),f.get("painted")) for f in sh])'
echo "shots: $OUT/shots"
