#!/bin/bash
# Suite 3: Live agent kicad_ipc co-editing (BeginCommit -> CreateItems -> EndCommit -> visual GAL
# repaint -> human Ctrl+Z undo), SIGSTOP child-freeze isolation + passive XGrabKey host chord rescue,
# host container hot-swap preserving unsaved edits, and SIGKILL crash recovery + clean teardown.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"

[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)

discover_bins
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub

PROJ="$OUT/proj"
API_SOCK="$OUT/tmp/kicad/api.sock"

cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"mark":"host_ready"},{"wait":120000}]
EOF

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 130 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

for _ in $(seq 80); do
  grep -q '"name":"host_ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break
  sleep 0.2
done
sleep 0.5

eval "$(python3 "$DRIVER_PY" find-gpui --shell --client-log "$OUT/client.log")"

start_container "$GPUI_XID" "$VX" "$VY" "$VW" "$VH" "c1"
CONT1_XID="$CONTAINER_LAST_XID"
CONT1_PID="$CONTAINER_LAST_PID"

# 1. Start ONE embedded pcbnew instance and connect to its live GUI IPC server
"$PCBNEW_BIN" "$PROJ/ecc83-pp.kicad_pcb" >"$OUT/pcbnew1.log" 2>&1 &
KP=$!
track_pid "$KP"

PCB_XID=$(wait_for_kicad_win "$KP" "PCB Editor" 80)
wait_for_ipc "$API_SOCK" 80
for _ in $(seq 40); do
  python3 "$IPC_PY" open-docs "$API_SOCK" pcb 2>/dev/null | grep -q '"count": 1' && break
  sleep 0.25
done

python3 "$DRIVER_PY" embed "$CONT1_XID" "$PCB_XID" "$VW" "$VH" >/dev/null
sleep 0.9

OPEN_DOCS=$(python3 "$IPC_PY" open-docs "$API_SOCK" pcb)
IPC_OK=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(int(d["count"]==1 and "ecc83-pp.kicad_pcb" in d["documents"][0]["board_filename"]))' "$OPEN_DOCS")
check "Embedded PCB Editor exposes live GUI IPC socket for ecc83-pp.kicad_pcb" "$IPC_OK" "$OPEN_DOCS"

# 2. Live agent BeginCommit -> CreateItems(Track) -> EndCommit -> visual GAL repaint -> human Ctrl+Z
CX=$((GX + VX + VW / 2))
CY=$((GY + VY + VH / 2))
python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
python3 "$XINPUT_PY" click "$CX" "$CY"
python3 "$XINPUT_PY" key Home
sleep 0.6

N0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
import -window root "$OUT/shots/01a_before_agent_commit.png"

ADD_RES=$(python3 "$IPC_PY" agent-add-track "$API_SOCK" 28.0 94.0 155.0 132.0 2.8 3)
sleep 0.7
import -window root "$OUT/shots/01b_after_agent_commit.png"

N1=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
MOD1=$(is_doc_modified "$API_SOCK" pcb "$PCB_XID")
AGENT_DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/01a_before_agent_commit.png" "$OUT/shots/01b_after_agent_commit.png" $((GX + VX + 40)) $((GY + VY + 40)) $((VW - 80)) $((VH - 80)))

LIVE_OK=$(python3 -c '
import json, sys
n0, n1, mod1 = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
diff = json.loads(sys.argv[4])
print(int(n1 == n0 + 1 and mod1 == 1 and diff["changed_pixels"] > 40))
' "$N0" "$N1" "$MOD1" "$AGENT_DIFF")
check "Live agent IPC commit incremented PCB traces ($N0 -> $N1) and visibly repainted embedded GAL canvas ($AGENT_DIFF)" "$LIVE_OK" "$AGENT_DIFF"

# Human presses Ctrl+Z inside embedded PCB Editor to undo the agent's staged commit in one step
python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
python3 "$XINPUT_PY" chord ctrl z
sleep 0.6
shot "02_after_human_undo"

N_UNDO=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
check "Human Ctrl+Z inside embedded PCB Editor atomically undid agent's IPC commit ($N1 -> $N_UNDO)" \
  "$([ "$N_UNDO" -eq "$N0" ] && echo 1 || echo 0)" \
  "expected $N0 got $N_UNDO"

# 3. Child freeze (SIGSTOP) isolation + passive XGrabKey host shortcut rescue
python3 "$IPC_PY" agent-add-track "$API_SOCK" 35.0 98.0 140.0 126.0 2.0 3 >/dev/null
sleep 0.35
python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null

kill -STOP "$KP"
GRAB_JSON=$(python3 "$DRIVER_PY" grab-chord-probe "$GPUI_XID" k "$XINPUT_PY" || echo '{"intercepted":false}')
HUB_ALIVE=0
curl -s -m 1 -o /dev/null "http://127.0.0.1:$HPORT/hello" && HUB_ALIVE=1
kill -CONT "$KP"

PING_AFTER_CONT=0
wait_for_ipc "$API_SOCK" 30 && PING_AFTER_CONT=1

FREEZE_OK=$(python3 -c '
import json, sys
g = json.loads(sys.argv[1])
print(int(bool(g.get("intercepted")) and g.get("elapsed_ms", 9999) < 1200 and int(sys.argv[2]) == 1 and int(sys.argv[3]) == 1))
' "$GRAB_JSON" "$HUB_ALIVE" "$PING_AFTER_CONT")
check "While KiCad is SIGSTOP-frozen, passive XGrabKey delivers Ctrl+K to GPUI host and hub stays responsive ($GRAB_JSON)" "$FREEZE_OK" "$GRAB_JSON"

# 4. Host container hot-swap (reparent dirty live PCB Editor into new container without losing unsaved edits)
start_container "$GPUI_XID" "$VX" "$VY" "$VW" "$VH" "c2"
CONT2_XID="$CONTAINER_LAST_XID"

python3 "$DRIVER_PY" embed "$CONT2_XID" "$PCB_XID" "$VW" "$VH" >/dev/null
stop_pid "$CONT1_PID"
sleep 0.6
shot "03_after_container_hotswap"

RPT_SWAP=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT2_XID" "$PCB_XID")
MOD_SWAP=$(is_doc_modified "$API_SOCK" pcb "$PCB_XID")
N_SWAP=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')

SWAP_OK=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1]); mod_s = int(sys.argv[2]); n_s = int(sys.argv[3]); n0 = int(sys.argv[4])
print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 20 and mod_s == 1 and n_s == n0 + 1))
' "$RPT_SWAP" "$MOD_SWAP" "$N_SWAP" "$N0")
check "Host container hot-swap reparented live KiCad cleanly and preserved unsaved dirty edits (traces=$N_SWAP)" "$SWAP_OK" "$RPT_SWAP"

# 5. Hard crash (SIGKILL) recovery & zero-orphan teardown
kill -KILL "$KP" 2>/dev/null || true
wait "$KP" 2>/dev/null || true
rm -f "$API_SOCK"
sleep 0.4

GPUI_AFTER_KILL=$(python3 "$DRIVER_PY" find-gpui --client-log "$OUT/client.log" || echo "")
HUB_AFTER_KILL=0
curl -s -m 1 -o /dev/null "http://127.0.0.1:$HPORT/hello" && HUB_AFTER_KILL=1
check "SIGKILL of embedded KiCad leaves Backplane GPUI window and hub alive without cascading crash" \
  "$([ -n "$GPUI_AFTER_KILL" ] && [ "$HUB_AFTER_KILL" -eq 1 ] && echo 1 || echo 0)"

# Re-open fresh replacement KiCad (still strictly 1 KiCad process alive) and re-embed
"$PCBNEW_BIN" "$PROJ/ecc83-pp.kicad_pcb" >"$OUT/pcbnew2.log" 2>&1 &
KP2=$!
track_pid "$KP2"

PCB2_XID=$(wait_for_kicad_win "$KP2" "PCB Editor" 80)
REOPEN_IPC=0
wait_for_ipc "$API_SOCK" 80 && REOPEN_IPC=1
python3 "$DRIVER_PY" embed "$CONT2_XID" "$PCB2_XID" "$VW" "$VH" >/dev/null
sleep 0.8
shot "04_reopened_after_crash"

RPT_REOPEN=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT2_XID" "$PCB2_XID")
REOPEN_OK=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1]); ipc = int(sys.argv[2])
print(int(ipc == 1 and bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 20))
' "$RPT_REOPEN" "$REOPEN_IPC")
check "Fresh KiCad instance re-opened and re-embedded cleanly after crash" "$REOPEN_OK" "$RPT_REOPEN"

stop_pid "$KP2"
stop_pid "$CP"
sleep 0.2

CLEAN_OK=1
kill -0 "$KP" 2>/dev/null && CLEAN_OK=0
kill -0 "$KP2" 2>/dev/null && CLEAN_OK=0
check "Clean teardown left zero running KiCad processes" "$CLEAN_OK"

finish_suite "embed_agent_resilience_e2e"
