#!/bin/bash
# https://github.com/BackplaneWorks/Backplane/issues/1326: a project with a schematic and no board (ecc83-pp's .kicad_pro and .kicad_sch,
# its .kicad_pcb removed) is warmed when a thread of it is shown (editor.kicad.warm, as the pane asks). Before the fix the hub warmed
# the board, and KiCad put "PCB '…/ecc83-pp.kicad_pcb' does not exist. Do you wish to create it?" over the app.
#   W1  the warm-up starts KiCad ({"warm": true, "started": true})
#   W2  the session comes up ready on the schematic (its active frame SCH)
#   W3  no KiCad dialog: none reported to the hub (ET_DIALOG_OPENED), no "Confirmation" window in the X tree, a few seconds after ready
#   W4  no board was made (no .kicad_pcb in the project)
# W3 and W4 count only after a real warm-up: the reply said started and the hub has a KiCad session for the project; otherwise they
# fail (with no warm-up there is no dialog and no board either way: a pass would prove nothing)
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_warm_sch_only_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"
# schematic only: the board and its companion go
( cd "$PROJ" && git rm -q -f ecc83-pp.kicad_pcb && { git rm -q -f ecc83-pp.kicad_pcb.backplane.json 2>/dev/null || true; } && git commit -qm "schematic only" )
[ ! -e "$PROJ/ecc83-pp.kicad_pcb" ] && [ -f "$PROJ/ecc83-pp.kicad_sch" ] || { echo "FAIL the fixture is not schematic-only"; exit 1; }
export BACKPLANE_KICAD_DEBUG=1
start_xvfb_and_wm; start_hub

# the hub's websocket answers only once it is serving (~4 s after it listens): wait for an empty round trip first
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done
# W1: the warm-up, as the pane asks it (no frame named)
hub_rpc "[{\"method\":\"editor.kicad.warm\",\"timeout_ms\":60000,\"params\":{\"project\":\"$PRO\",\"display\":\"${DISPLAY:-}\"}}]" >"$OUT/warm.out" 2>&1 || true
W=$(grep '"m":"editor.kicad.warm"' "$OUT/warm.out" | tail -1 || true)
echo "INFO warm: $(echo "$W" | cut -c1-300)"
STARTED=$(echo "$W" | grep -q '"warm":true' && echo "$W" | grep -q '"started":true' && echo 1 || echo 0)
check "W1 the warm-up starts KiCad" "$STARTED" "$(echo "$W" | cut -c1-200)"

# W2: ready, on the schematic
STATE=""; ACTIVE=""
for _ in $(seq 240); do
  read -r STATE ACTIVE < <(hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: v=json.load(sys.stdin).get("value") or {}; print(v.get("state","-"), v.get("active","-"))
except Exception: print("- -")' 2>/dev/null || echo "- -")
  [ "$STATE" = ready ] && break
  sleep 0.5
done
check "W2 the session is ready on the schematic" "$([ "$STATE" = ready ] && [ "$ACTIVE" = SCH ] && echo 1 || echo 0)" "state $STATE, active $ACTIVE"
# a warm-up that happened: started, and a KiCad session of the project (starting or ready)
WARMED=$([ "$STARTED" = 1 ] && { [ "$STATE" = ready ] || [ "$STATE" = starting ]; } && echo 1 || echo 0)

# W3: no dialog, a few seconds after ready (KiCad's question came as it opened the board)
sleep 5
xwininfo -root -tree >"$OUT/xtree.txt" 2>&1 || true
DLG_X=$(grep -ciE '"Confirmation"|does not exist' "$OUT/xtree.txt" || true)
DLG_H=$(grep -c 'ET_DIALOG_OPENED' "$OUT/hub.log" || true)
shot w3_after_warm 2>/dev/null || true
check "W3 no KiCad dialog (none reported, no Confirmation window), after a real warm-up" "$([ "$WARMED" = 1 ] && [ "${DLG_X:-0}" = 0 ] && [ "${DLG_H:-0}" = 0 ] && echo 1 || echo 0)" "X tree ${DLG_X:-0}, hub ${DLG_H:-0}: $(grep -o 'ET_DIALOG_OPENED[^}]*' "$OUT/hub.log" | head -1)"

# W4: nothing made
check "W4 no board was made, after a real warm-up" "$([ "$WARMED" = 1 ] && [ ! -e "$PROJ/ecc83-pp.kicad_pcb" ] && echo 1 || echo 0)" "warmed $WARMED (started $STARTED, state $STATE)"
hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >/dev/null 2>&1 || true
finish_suite "embed_warm_sch_only_e2e ($EMBED_MODE)"
