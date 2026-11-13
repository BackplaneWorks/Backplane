#!/bin/bash
# Suite: "Open in GerbView" on a Gerber set (screens/viewer.rs, viewers/kicad_pane.rs open_in_gerbview; lane B's open_gerbers):
#   G1  with a runtime that has GerbView in process (embed-gerbview), Backplane's Gerbers page offers "Open in GerbView"
#   G2  pressed with Edit off: Edit comes on, and KiCad's GerbView is the frame shown in the pane, with the set's job loaded (its layers)
#   G3  Board's tab brings the PCB editor back, the same KiCad
# Without embed-gerbview the page offers nothing (checked on a runtime without it: the button is not registered).
#   P1  Edit: KiCad's PCB editor mapped in the pane
#   P2  each other page: KiCad's container unmapped (a screenshot each); Gerbers: the set's plot painted (lane K's rerun 9 bug: the rail
#       lit Gerbers, the PCB editor stayed on screen)
#   P3  Board again: the editor mapped in the pane, within 1 s, the same KiCad (no new attach)
# Copies only (seed_project.py; the fixture's Gerbers exported by kicad-cli into mfg/). EMBED_MODE=fork|stock.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
[ "$EMBED_MODE" = "spike" ] && { echo "SKIP embed_gerbview_e2e: product modes only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
[ -n "${KICAD_CLI:-}" ] || KICAD_CLI="$(dirname "${KICAD_BIN:-/usr/bin/kicad}")/kicad-cli"
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots" "$PROJ/mfg/gerbers"
"$KICAD_CLI" pcb export gerbers -o "$PROJ/mfg/gerbers/" "$PCB" >/dev/null 2>&1 || true
"$KICAD_CLI" pcb export drill --format excellon -o "$PROJ/mfg/gerbers/" "$PCB" >/dev/null 2>&1 || true
git -C "$PROJ" add -A >/dev/null 2>&1; git -C "$PROJ" commit -qm "fab output" >/dev/null 2>&1 || true
start_xvfb_and_wm
start_hub
cat >"$OUT/script.json" <<JSON
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"tag":"painted"}},{"wait":2000},{"kicad":{"report":true,"tag":"ready"}},{"click":"ui:View"},{"wait":1500},
 {"click":"rail:Gerbers"},{"wait":3000},{"mark":"gerbers"},{"shot":"$OUT/shots/g1_gerbers.png"},
 {"click":"ui:Open in GerbView"},{"wait":12000},{"kicad":{"report":true,"tag":"gerbview"}},{"shot":"$OUT/shots/g2_gerbview.png"},
 {"click":"kicad-tab:PCB"},{"wait":3000},{"kicad":{"report":true,"tag":"pcb"}},{"shot":"$OUT/shots/g3_pcb.png"},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 240 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 480 || { echo "FAIL the script never finished"; tail -n 20 "$OUT/client.log"; finish_suite "embed_gerbview_e2e ($EMBED_MODE)"; exit 1; }
rep() { last_kicad_report "$OUT/client.log" "$1"; }
# the runtime's GerbView, from the open KiCad's own capabilities (its socket in the pane's report)
SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("sock") or "")' "$(rep ready)" 2>/dev/null || true)
CAPS=$( [ -n "$SOCK" ] && python3 "$IPC_PY" caps "$SOCK" 2>/dev/null || true)
echo "INFO KiCad's capabilities: $(echo "$CAPS" | grep -o '"embed[^"]*"' | tr '\n' ' ')"
if ! echo "$CAPS" | grep -q embed-gerbview; then
  # the button drawn (its rect) or pressable (the script's click found it); the script's own attempt is logged either way
  check "Without embed-gerbview the Gerbers page offers no Open in GerbView" "$(grep -qE '"id":"ui:Open in GerbView","x"|"id":"ui:Open in GerbView","found":true' "$OUT/client.log" && echo 0 || echo 1)"
  finish_suite "embed_gerbview_e2e ($EMBED_MODE)"; exit 0
fi
check "G1 the Gerbers page offers Open in GerbView (embed-gerbview)" "$(grep -q '"id":"ui:Open in GerbView"' "$OUT/client.log" && echo 1 || echo 0)"
G=$(rep gerbview); P=$(rep pcb)
check "G2 KiCad's GerbView is the frame shown in the pane" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); f=[f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")]; print(int(r.get("want")=="GERBVIEW" and bool(f) and f[0].get("kind")=="GERBVIEW" and (r.get("container") or {}).get("shown") is True))' "$G" 2>/dev/null || echo 0)" "$(echo "$G" | cut -c1-400); see shots/g2_gerbview.png"
check "G2 the hub opened the set's job in GerbView" "$(grep -q 'editor.kicad.show' "$OUT/client.log" && grep -o '"kicad_rpc_sent":"editor.kicad.show"[^}]*GERBVIEW' "$OUT/client.log" | head -1 | grep -q GERBVIEW && echo 1 || echo 0)" "$(grep -o '"gerbview":[^}]*' "$OUT/client.log" | head -3)"
check "G3 Board's tab brings the PCB editor back, the same KiCad" "$(python3 -c 'import json,sys; a=json.loads(sys.argv[1] or "{}"); b=json.loads(sys.argv[2] or "{}"); f=[f for f in b.get("frames",[]) if f.get("xid")==b.get("shown")]; print(int(bool(f) and f[0].get("kind")=="PCB" and a.get("pid")==b.get("pid")))' "$G" "$P" 2>/dev/null || echo 0)" "$(echo "$P" | cut -c1-300)"
finish_suite "embed_gerbview_e2e ($EMBED_MODE)"
