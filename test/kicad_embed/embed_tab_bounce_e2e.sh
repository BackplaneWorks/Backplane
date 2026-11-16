#!/bin/bash
# Ctrl+Tab (and the Schematic tab) to a schematic editor opening cold: the pane stays on the schematic (lane M, Mac mini: the schematic's
# library load made KiCad report the board active again ~300 ms after the switch, and the pane went back to it; the next Ctrl+Shift+Tab
# then went board -> GerbView). No pre-warm, so the schematic opens cold; Hubble_DK (TAB_BOARD) by default, a schematic slow to load.
#   T1  Ctrl+Tab in KiCad from the board: the pane shows the schematic and stays there for 5 s (no kicad_tab back to PCB)
#   T2  after it, Ctrl+Shift+Tab goes back to the board (not to another editor)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_tab_bounce_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
BOARD=${TAB_BOARD:-$REAL_HOME/Documents/Nordic/Hubble_DK}
[ -d "$BOARD" ] || { echo "SKIP embed_tab_bounce_e2e: no board at $BOARD (TAB_BOARD)"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"
( cd "$PROJ" && git rm -q -r --cached . >/dev/null && find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} + )
rsync -a --exclude '*backups*' --exclude '~*.lck' --exclude '.git' --exclude 'mfg' --exclude 'build' "$BOARD"/ "$PROJ"/
echo "{\"project\": \"$(basename "$(ls "$PROJ"/*.kicad_pro | head -1)")\"}" >"$PROJ/.backplane.json"
git -C "$PROJ" add -A >/dev/null 2>&1; git -C "$PROJ" commit -qm "the board" >/dev/null 2>&1 || true
# no pre-warm: the schematic editor opens when it is asked for, cold
export BACKPLANE_KICAD_PREWARM_FREE_MB=100000000 BACKPLANE_KICAD_DEBUG=1
start_xvfb_and_wm; start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":120000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 900 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
read -r X Y W H < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
sleep 3
python3 "$XINPUT_PY" click $((X + W / 2)) $((Y + H - 120)); sleep 0.6; python3 "$XINPUT_PY" key Escape; sleep 0.4
# the pane's tab changes from here on (kicad_tab: from, to, cause)
tabs() { tail -n +"$1" "$OUT/client.log" | python3 -c 'import json,sys
for l in sys.stdin:
    if "\"kicad_tab\"" in l:
        t=json.loads(l.split("GPUI_LOG ",1)[1])["kicad_tab"]; print(t.get("from"), "->", t.get("to"), "|", t.get("cause"))'; }
N=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" chord ctrl Tab; sleep 6
echo "INFO the pane's tab changes after Ctrl+Tab:"; tabs "$N" | sed 's/^/INFO   /'
FIRST=$(tabs "$N" | head -1); BACK=$(tabs "$N" | grep -c -- "-> PCB" || true)
check "T1 Ctrl+Tab to the cold schematic: the pane goes to it and stays (no switch back to the board within 6 s)" "$(echo "$FIRST" | grep -q -- "-> SCH" && [ "$BACK" = 0 ] && echo 1 || echo 0)" "first: $FIRST; back to the board $BACK times"
N=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" chord ctrl shift Tab; sleep 3
T2=$(tabs "$N" | head -1)
check "T2 then Ctrl+Shift+Tab goes back to the board" "$(echo "$T2" | grep -q -- "SCH -> PCB" && echo 1 || echo 0)" "$T2"
echo "INFO the hub's active frame around it: $(grep -o '"kicad_info":"[^"]*","gone":false,"state":"ready","mode":"fork","pid":[0-9]*,"active":"[A-Z_]*"' "$OUT/client.log" | tail -8 | grep -o '"active":"[A-Z_]*"' | tr '\n' ' ')"
kill "$CP" 2>/dev/null || true
finish_suite "embed_tab_bounce_e2e ($EMBED_MODE)"
