#!/bin/bash
# Real use, measured: Edit on a copy of a real board, then the journeys of docs/kicad-real-use.md (reopen, tab and thread switches with
# the pane's pixels sampled for flashes, pan/zoom frame times, select/move/delete/undo/route, an agent's commits idle and during a pan,
# editor.kicad.select, Comment to Backplane, Ctrl+S, the schematic). One board x runtime per call; real_use_all.sh runs the matrix.
#   REAL_HARDWARE=1 EMBED_MODE=fork BACKPLANE_KICAD_BIN=.../bin/kicad KEMBED_PROJ_SRC=<staged copy> test/kicad_embed/real_use_journeys.sh
#   KEMBED_JOURNEYS=focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,tools,sch (default all)
# Prints PERF lines and a JSON line; writes $OUT/real_use.json.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "real_use_journeys: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
KEMBED_THREADS=2 python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
cat >"$OUT/script.json" <<'J'
[{"wait":1500},{"click":"project:p1"},{"wait":1200},{"click":"page-thread:t1"},{"wait":1500},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"cold"}},
 {"wait":3500},{"kicad":{"report":true,"tag":"geom"}},{"mark":"ready_for_driver"},{"wait":1500000}]
J
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 1600 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
export IPC_PY DRIVER_PY HUB_RPC_MJS HPORT REPO_ROOT REAL_HOME
RC=0
python3 "$KEMBED_LIB/real_use_drive.py" "$OUT" "$OUT/client.log" "${KEMBED_JOURNEYS:-focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,tools,sch}" || RC=$?
shot end
stop_pid "$CP"
echo "real_use_journeys: rc=$RC artifacts=$OUT"
exit "$RC"
