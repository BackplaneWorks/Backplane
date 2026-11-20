#!/bin/bash
# #1345: a KiCad the person left by switching threads is out of the pane for the idle close (BACKPLANE_KICAD_IDLE_S=20 here; 300 s
# by default), kept warm meanwhile, and the same process when they come back in time. One client, two threads (KEMBED_THREADS=2):
#   L2  away from the editor's thread 6 s and back: the same KiCad pid; the client sent exactly one leave and one back, and a quick
#       away-and-back before it sent none
#   L4  away while an agent keeps reading the board in that KiCad past the idle time: the same pid (use is not idle)
#   L1  then nothing for longer than the idle time: KiCad closed as idle, and the hub log says why ("left the pane: thread")
#   L5  back to the thread after the idle close: Backplane's viewer (no pane on a KiCad that is gone), no error notice
#   L3  Edit again, an agent's unsaved track, away past the idle time: the same pid (unsaved work is kept)
# Each fails on main (no leave there: nothing closes after a thread switch, and nothing is counted). Fork only; copies (seed_project.py).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_leave_idle_e2e: fork only"; exit 0; }
export KEMBED_THREADS=2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1); PCB=${PRO%.kicad_pro}.kicad_pcb
IDLE=20
export BACKPLANE_KICAD_IDLE_S=$IDLE BACKPLANE_KICAD_WARM_HEADROOM_MB=0
start_xvfb_and_wm; start_hub
echo "INFO hub on $HPORT, project $PRO, idle time $IDLE s"
kpid() { pgrep -u "$(id -u)" -f -- "--embedded.*--parent-pid.*$(basename "$PRO")" 2>/dev/null | tail -1 || true; }
ask() { local want=$2 before n; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true); before=${before:-0}
  local text; text=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mlv$(date +%s%N)\"}}]" >>"$OUT/rpc.log" 2>&1
  for _ in $(seq 360); do n=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true); [ "${n:-0}" -ge $((before + want)) ] && break; sleep 0.25; done; sleep 0.5; }
DOC="{\"type\":\"DOCTYPE_PCB\",\"board_filename\":\"$(basename "$PCB")\"}"
agent_read() { ask "mcpcall:{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.GetItems\",\"message\":{\"header\":{\"document\":$DOC},\"types\":[\"KOT_PCB_FOOTPRINT\"]}}}" 1; }

python3 - "$OUT/script.json" <<'PY'
import json, sys
# the sidebar lists the newest thread first: 0 is "Other thread 2" (t2), 1 is "Embedded KiCad E2E" (t1), where the editor is opened
s = [{"select": 1}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "edit"}}, {"wait": 3000}, {"mark": "edited"},
     {"select": 0}, {"select": 1}, {"wait": 1500}, {"mark": "quick"},
     {"select": 0}, {"wait": 6000}, {"mark": "away1"}, {"select": 1}, {"kicad": {"wait_painted": 8000, "kind": "PCB", "tag": "back1"}}, {"wait": 1500}, {"mark": "back1"},
     {"select": 0}, {"mark": "agentaway"}, {"wait": 40000}, {"mark": "agentdone"}, {"wait": 40000}, {"mark": "idlewait"},
     {"select": 1}, {"wait": 3000}, {"kicad": {"report": True, "tag": "l5"}}, {"mark": "l5"},
     {"click": "ui:Edit"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "edit2"}}, {"mark": "edit2"}, {"wait": 25000},
     {"select": 0}, {"mark": "unsavedaway"}, {"wait": 40000}, {"mark": "unsaveddone"}, {"wait": 1000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 400 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mark() { wait_for_log "$OUT/client.log" "\"name\":\"$1\",\"event\":\"mark\"" "${2:-1800}"; }
leaves() { grep -c "\"kicad_leave\":$1" "$OUT/client.log" 2>/dev/null || true; }

mark edited || { echo "INFO the editor did not come"; }
PID0=$(kpid); echo "INFO KiCad $PID0"
mark quick || true
Q_LEAVE=$(leaves true); Q_BACK=$(leaves false)
mark back1 || true
PID1=$(kpid); N_LEAVE=$(leaves true); N_BACK=$(leaves false)
echo "INFO quick away-and-back: $Q_LEAVE leave(s) $Q_BACK back(s); after 6 s away and back: $N_LEAVE leave(s) $N_BACK back(s); pid $PID0 -> $PID1"
check "L2 away 6 s and back: the same KiCad, one leave and one back (none for the quick away-and-back)" "$([ -n "$PID0" ] && [ "$PID1" = "$PID0" ] && [ "$Q_LEAVE" = 0 ] && [ "$Q_BACK" = 0 ] && [ "$N_LEAVE" = 1 ] && [ "$N_BACK" = 1 ] && echo 1 || echo 0)" "pid $PID0 -> $PID1; quick $Q_LEAVE/$Q_BACK; then $N_LEAVE/$N_BACK"

mark agentaway || true
T0=$(date +%s); READS=0
while [ $(( $(date +%s) - T0 )) -lt $((IDLE + 15)) ]; do agent_read; READS=$((READS + 1)); sleep 4; done
PID4=$(kpid)
echo "INFO an agent read the board $READS times over $(( $(date +%s) - T0 )) s away; pid $PID0 -> ${PID4:-gone}"
check "L4 away while an agent keeps using that KiCad past the idle time: the same pid" "$([ -n "$PID4" ] && [ "$PID4" = "$PID0" ] && [ "$READS" -ge 5 ] && echo 1 || echo 0)" "pid $PID0 -> ${PID4:-gone}, $READS reads"

mark idlewait || true
PIDI=$(kpid); WHY=$(grep -o "kicad embed: idle, closing [^ ]*$(basename "$PRO") ([^)]*)" "$OUT/hub.log" | head -1 || true)
echo "INFO after the agent stopped: pid ${PIDI:-gone}; hub: ${WHY:-no idle close}"
check "L1 left and unused past the idle time: closed as idle, the hub log says it left the pane by a thread switch" "$([ -z "$PIDI" ] && echo "$WHY" | grep -q "left the pane: thread" && echo 1 || echo 0)" "pid ${PIDI:-gone}; $WHY"

mark l5 || true
R=$(last_kicad_report "$OUT/client.log" l5)
read -r PANE NOTICE < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(bool(r.get("project")) and bool(r.get("pane"))), json.dumps(r.get("notice")))' "${R:-{\}}")
AWAYV=$(grep -c '"kicad_idle_closed_away"' "$OUT/client.log" || true)
echo "INFO back after the idle close: pane $PANE, notice $NOTICE, viewer given back $AWAYV; $(echo "$R" | cut -c1-200)"
check "L5 back after the idle close: Backplane's viewer, no pane on a gone KiCad, no error notice" "$([ "$PANE" = 0 ] && [ "$AWAYV" -ge 1 ] && { [ "$NOTICE" = null ] || [ -z "$NOTICE" ]; } && echo 1 || echo 0)" "pane $PANE notice $NOTICE away $AWAYV"

mark edit2 || true
PID5=$(kpid)
TRACK="{\"@type\":\"type.googleapis.com/kiapi.board.types.Track\",\"start\":{\"x_nm\":\"60000000\",\"y_nm\":\"60000000\"},\"end\":{\"x_nm\":\"70000000\",\"y_nm\":\"60000000\"},\"width\":{\"value_nm\":\"300000\"},\"layer\":\"BL_F_Cu\"}"
ask "mcpcall:{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"save\":false,\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":$DOC},\"items\":[$TRACK]}}}" 1
LOCK=$(grep "^MCPCALL kicad_ipc" "$OUT/standin.log" | tail -1 | grep -o 'lock_acquire kicad-[A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2 || true)
[ -n "$LOCK" ] && ask "mcpseq:[{\"name\":\"lock_acquire\",\"arguments\":{\"name\":\"$LOCK\"}},{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"save\":false,\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":$DOC},\"items\":[$TRACK]}}}]" 2
echo "INFO the unsaved edit: $(grep '^MCPCALL kicad_ipc' "$OUT/standin.log" | tail -1 | cut -c1-160)"
UNS=$(hub_rpc "[]" 2>/dev/null | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | python3 -c 'import json,sys
try: v=json.loads(sys.stdin.read()[5:]).get("value") or {}
except Exception: v={}
if isinstance(v, str): v=json.loads(v)
print(len(v.get("agent_unsaved") or []) + len(v.get("unsaved") or []))' 2>/dev/null || echo 0)
mark unsaveddone || true
PID6=$(kpid)
echo "INFO Edit again: KiCad $PID5 (unsaved docs in the info: $UNS); away past the idle time: ${PID6:-gone}"
check "L3 unsaved work, away past the idle time: the same KiCad kept" "$([ -n "$PID5" ] && [ "$PID6" = "$PID5" ] && echo 1 || echo 0)" "pid $PID5 -> ${PID6:-gone}"

echo "INFO hub: $(grep -c 'left the pane (' "$OUT/hub.log" || true) leave(s), $(grep -c 'back in the pane' "$OUT/hub.log" || true) back(s), $(grep -c 'not the pane.s holder' "$OUT/hub.log" || true) refused"
grep -E "kicad embed: .*(left the pane|back in the pane|idle, closing)" "$OUT/hub.log" | cut -c1-220 | sed 's/^/INFO   /' || true
wait "$CP" 2>/dev/null || true
finish_suite "embed_leave_idle_e2e ($EMBED_MODE)"
