#!/bin/bash
# A click on one of KiCad's DRC markers in the embedded editor, as a person makes it (XTest press and release on the canvas), on a real
# board (MARKER_BOARD, default Hubble_DK, whose zoomed-to-fit centre is a marker: real_use's select click, lane A's repro).
#   M1  the click leaves the editor where it was: KiCad's frame keeps its place and size, the Checks panel stays shut (it opened beside
#       the editor, narrowed it and slid the canvas under the pointer: the person's next press, meant for the board, landed on the panel,
#       the keyboard went to Backplane, Delete and Ctrl+Z reached nothing, and the next Escape closed the viewer with the editor in it)
#   M2  the strip's checks chip says the finding for a few seconds
#   M3  Escape twice (leaving a KiCad tool) keeps the editor shown, and Delete then reaches KiCad (the board becomes modified)
#   M4  a press on the chip opens Checks on the clicked row
#   M5  Escape in Backplane shuts the open panel first, the viewer stays; a second Escape closes the viewer, and KiCad keeps the unsaved
#       edit (the session is not closed under it)
# Fork mode; skips without MARKER_BOARD on disk or without embed-checks.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_marker_click_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
BOARD=${MARKER_BOARD:-$REAL_HOME/Documents/Nordic/Hubble_DK}
[ -d "$BOARD" ] || { echo "SKIP embed_marker_click_e2e: no board at $BOARD (MARKER_BOARD)"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"
( cd "$PROJ" && git rm -q -r --cached . >/dev/null && find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} + )
python3 "$KEMBED_LIB/project_copy.py" "$BOARD" "$PROJ" --skip '*backups*' --skip mfg --skip build
echo "{\"project\": \"$(basename "$(ls "$PROJ"/*.kicad_pro | head -1)")\"}" >"$PROJ/.backplane.json"
git -C "$PROJ" add -A >/dev/null 2>&1; git -C "$PROJ" commit -qm "the board" >/dev/null 2>&1 || true
PRO=$(ls "$PROJ"/*.kicad_pro | head -1); PCB="${PRO%.kicad_pro}.kicad_pcb"; mkdir -p "$OUT/shots"; export PRO OUT HUB_RPC_MJS
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
export HPORT

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"tag":"painted"}},{"wait":3000},{"kicad":{"report":true,"tag":"ready"}},{"mark":"ready"},{"wait":400000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 480 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("sock") or "")' "$R")
CAPS=$(python3 "$IPC_PY" caps "$SOCK" 2>>"$OUT/ipc.err" || echo '{}')
echo "$CAPS" | grep -q 'embed-checks' || { echo "SKIP embed_marker_click_e2e: the editor has no embed-checks"; finish_suite "embed_marker_click_e2e ($EMBED_MODE)"; exit 0; }

# DRC in the open editor (its markers drawn in the canvas, its rows the hub's), the Checks panel shut
T0=$(python3 -c 'import time; print(int(time.time()*1000))')
hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\"}}]" >>"$OUT/rpc.log" 2>&1 || true
for _ in $(seq 480); do
  S=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" 2>/dev/null | grep -v '^INFO ' | head -1)
  python3 -c 'import json,sys; d=json.loads(sys.argv[1])["result"]["drc"]; sys.exit(0 if d["state"]=="fresh" and d["in_editor"] and d["at"] > int(sys.argv[2]) else 1)' "$S" "$T0" 2>/dev/null && break
  sleep 0.5
done
echo "INFO DRC in the editor: $(echo "$S" | cut -c1-200)"

python3 "$KEMBED_LIB/marker_click_drive.py" "$OUT" "$R" "$SOCK" "$PCB" "$IPC_PY" "$XINPUT_PY" >"$OUT/drive.log" 2>&1 || true
cat "$OUT/drive.log"
while read -r line; do
  case "$line" in CHECK\ *) v=${line#CHECK }; ok=${v%% *}; rest=${v#* }; check "${rest%%|*}" "$ok" "${rest#*|}";; esac
done <"$OUT/drive.log"
grep -q '^CHECK ' "$OUT/drive.log" || check "the drive ran" 0 "$(tail -5 "$OUT/drive.log")"
kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_marker_click_e2e ($EMBED_MODE)"
