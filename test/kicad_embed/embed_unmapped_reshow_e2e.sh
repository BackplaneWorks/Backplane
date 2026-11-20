#!/bin/bash
# #1403: the pane's "placed frame stayed unmapped" check (kicad_pane.rs, now nc::mapped: visibility only, no pixel read) still fires: after
# Edit, the board editor's frame is unmapped behind the pane's back (XWithdrawWindow, as a lost map would leave it); the pane asks the hub to
# show it again (editor.kicad.show) and it is mapped again within 5 s. Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_unmapped_reshow_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"wait":3000},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},{"wait":120000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 300 || { echo "FAIL the board never opened in KiCad"; tail -n 30 "$OUT/client.log"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
FRAME=$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1]).get("shown") or 0))' "$R")
[ "$FRAME" -gt 0 ] || { echo "FAIL no frame on show in the report"; exit 1; }
mapped() { python3 "$DRIVER_PY" list-windows 2>/dev/null | python3 -c 'import json,sys
x=int(sys.argv[1]); print(int(any(w.get("xid_int")==x and w.get("shown") for w in json.load(sys.stdin))))' "$1" 2>/dev/null || echo 0; }
M0=$(mapped "$FRAME")
L=$(wc -l <"$OUT/client.log")
python3 "$DRIVER_PY" withdraw "$(printf '0x%x' "$FRAME")" >/dev/null
sleep 0.3; M1=$(mapped "$FRAME")
back=0; for _ in $(seq 50); do [ "$(mapped "$FRAME")" = 1 ] && { back=1; break; }; sleep 0.1; done
ASKED=$(tail -n +"$((L + 1))" "$OUT/client.log" | grep -ac '"kicad_rpc_sent":"editor.kicad.show"' || true)
echo "INFO frame $FRAME mapped before $M0, right after the withdraw $M1, shows asked after it: ${ASKED:-0}"
check "U0 the board's frame was on show, and the withdraw unmapped it" "$([ "$M0" = 1 ] && [ "$M1" = 0 ] && echo 1 || echo 0)"
check "U1 the pane saw it unmapped and asked the hub to show it again (editor.kicad.show)" "$([ "${ASKED:-0}" -ge 1 ] && echo 1 || echo 0)"
check "U2 the board's frame is mapped again within 5 s" "$back"
hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" >/dev/null || true
finish_suite "embed_unmapped_reshow_e2e ($EMBED_MODE)"
