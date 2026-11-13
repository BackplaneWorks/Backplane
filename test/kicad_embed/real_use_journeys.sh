#!/bin/bash
# Real use, measured: Edit on a copy of a real board, then the journeys of docs/kicad-real-use.md (reopen, tab and thread switches with
# the pane's pixels sampled for flashes, pan/zoom frame times, select/move/delete/undo/route, an agent's commits idle and during a pan,
# editor.kicad.select, Comment to Backplane, Ctrl+S, the schematic). One board x runtime per call; real_use_all.sh runs the matrix.
#   REAL_HARDWARE=1 EMBED_MODE=fork BACKPLANE_KICAD_BIN=.../bin/kicad KEMBED_PROJ_SRC=<staged copy> test/kicad_embed/real_use_journeys.sh
#   KEMBED_JOURNEYS=focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,tools,textkeys,editors,undo,checks,markers,panels,sch (default all)
# Prints PERF lines and a JSON line; writes $OUT/real_use.json.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "real_use_journeys: fork only"; exit 0; }
# A pinned client needs a pinned hub: discover_bins would otherwise take the worktree's own target/fast hub (an older build: Nov 13's
# gal2/gal3 ran client 46cd43ec0 with a 015eb3206 hub whose immediate in-KiCad check switched the pane to the board)
[ -z "${CLIENT_BIN:-}" ] || [ -n "${HUB_BIN:-}" ] || { echo "FAIL real_use_journeys: CLIENT_BIN is pinned but HUB_BIN is not; pin both"; exit 2; }
discover_bins; discover_kicad; init_isolated_dirs
bin_id() {  # path, sha256, mtime and the build folder's COMMIT file, when it has one
  local c; c=$(cat "$(dirname "$1")/COMMIT" 2>/dev/null | head -1 || true)
  echo "path=$1 sha256=$(sha256sum <"$1" | cut -c1-16) mtime=$(date -r "$1" +%FT%T) commit=${c:-unknown}"
}
echo "INFO hub $(bin_id "$HUB_BIN")"; echo "INFO client $(bin_id "$CLIENT_BIN")"
export HUB_ID="$(bin_id "$HUB_BIN")" CLIENT_ID="$(bin_id "$CLIENT_BIN")"
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
python3 "$KEMBED_LIB/real_use_drive.py" "$OUT" "$OUT/client.log" "${KEMBED_JOURNEYS:-focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,tools,textkeys,editors,undo,checks,markers,panels,sch}" || RC=$?
shot end
stop_pid "$CP"
# the client's memory over the run (env_xvfb rss_watch): a runaway is a failure here, never the machine's OOM killer
RV=$(rss_verdict || true)
if [ -n "$RV" ]; then
  echo "PERF client_memory $(echo "$RV" | head -1)"
  OKS=$(echo "$RV" | tail -1)
  [ "${OKS%% *}" = 1 ] || { echo "FAIL the client's memory stayed under ${RSS_MAX_MB:-2048} MB ($(echo "$RV" | head -1))"; RC=1; }
  [ "${OKS##* }" = 1 ] || { echo "FAIL the client's memory did not grow more than ${RSS_GROWTH_MB:-768} MB over the run ($(echo "$RV" | head -1))"; RC=1; }
fi
echo "real_use_journeys: rc=$RC artifacts=$OUT"
exit "$RC"
