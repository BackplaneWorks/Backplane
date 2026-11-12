#!/bin/bash
# Suite 3: Live agent kicad_ipc co-editing (BeginCommit -> CreateItems -> EndCommit -> visual GAL
# repaint -> human Ctrl+Z undo), SIGSTOP child-freeze isolation + passive XGrabKey host chord rescue,
# dirty-close protection + GPUI client crash survival / container re-adoption preserving unsaved edits,
# and KiCad SIGKILL crash recovery (editor.kicad.restart) + clean zero-orphan teardown.
#
# Modes:
#   EMBED_MODE=stock|fork — drives Backplane's own ui:Edit, grab_escape, save-set survival, and hub RPCs
#   EMBED_MODE=spike      — Phase 1 X11 feasibility harness
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

if [ "$EMBED_MODE" = "spike" ]; then
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

  python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
  python3 "$XINPUT_PY" chord ctrl z
  sleep 0.6
  shot "02_after_human_undo"

  N_UNDO=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
  KDESC="$(dirname "$KICAD_BIN")/../share/backplane/api.desc"
  if grep -qa "UndoAgentChange" "$KDESC" 2>/dev/null; then
    # bp.11 co-edit, per-actor undo: the person's Ctrl+Z leaves the agent's edit alone; the person takes it back from the Agents pane
    # (the same as UndoAgentChange over IPC)
    check "Human Ctrl+Z leaves the agent's IPC commit alone (per-actor undo: $N1 -> $N_UNDO)" "$([ "$N_UNDO" -eq "$N1" ] && echo 1 || echo 0)" "expected $N1 got $N_UNDO"
    UA=$(python3 "$IPC_PY" undo-agent-latest "$API_SOCK" 2>&1 || true); sleep 0.6
    N_UNDO=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
    check "The agent's change taken back (UndoAgentChange, as the Agents pane does: $N1 -> $N_UNDO)" "$([ "$N_UNDO" -eq "$N0" ] && echo 1 || echo 0)" "expected $N0 got $N_UNDO: $UA"
  else
    check "Human Ctrl+Z inside embedded PCB Editor atomically undid agent's IPC commit ($N1 -> $N_UNDO)" \
      "$([ "$N_UNDO" -eq "$N0" ] && echo 1 || echo 0)" \
      "expected $N0 got $N_UNDO"
  fi

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

  # 4. Host container hot-swap
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

  finish_suite "embed_agent_resilience_e2e (spike)"
  exit $?
fi

# ------------------------------------------------------------------------------
# Product mode (stock | fork): ZERO driver placement!
# Drives Backplane's own ui:Edit, grab_escape, XFixes save-set, and hub RPCs.
# ------------------------------------------------------------------------------
cat >"$OUT/script1.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":45000,"kind":"PCB","tag":"pcb_init"}},{"mark":"host_ready"}]
EOF

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script1.json" --exit-after 180 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

wait_for_log "$OUT/client.log" '"name":"host_ready","event":"mark"' 240
R_INIT=$(last_kicad_report "$OUT/client.log" "pcb_init")
read -r KP API_SOCK GPUI_XID CONT1_XID PCB_XID < <(python3 - "$R_INIT" <<'PY'
import json, sys
r = json.loads(sys.argv[1])
pid = int(r.get("pid") or 0)
sock = r.get("sock") or ""
top = f"0x{int(r.get('toplevel') or 0):x}"
cont = f"0x{int((r.get('container') or {}).get('xid') or 0):x}"
shown = f"0x{int(r.get('shown') or 0):x}"
print(pid, sock or "-", top, cont, shown)
PY
)
[ "$API_SOCK" = "-" ] && API_SOCK=""
[ "$KP" -gt 0 ] && track_pid "$KP"

wait_for_ipc "$API_SOCK" 80
for _ in $(seq 50); do
  python3 "$IPC_PY" open-docs "$API_SOCK" pcb 2>/dev/null | grep -q '"count": 1' && break
  sleep 0.25
done

OPEN_DOCS=$(python3 "$IPC_PY" open-docs "$API_SOCK" pcb)
IPC_OK=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(int(d["count"]==1 and "ecc83-pp.kicad_pcb" in d["documents"][0]["board_filename"]))' "$OPEN_DOCS")
check "Backplane-launched PCB Editor ($EMBED_MODE) exposes live GUI IPC socket for ecc83-pp.kicad_pcb" "$IPC_OK" "$OPEN_DOCS"

# 2. Live agent BeginCommit -> CreateItems(Track) -> EndCommit -> visual GAL repaint -> human Ctrl+Z
RPT_PCB=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT1_XID" "$PCB_XID")
read -r BX BY BW BH < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT_PCB")
GAL_X=$((BX + BW * 2 / 3))
GAL_Y=$((BY + BH / 2))

python3 "$XINPUT_PY" click "$GAL_X" "$GAL_Y"
sleep 0.35
python3 "$XINPUT_PY" key Home
sleep 0.6

N0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
import -window root "$OUT/shots/01a_before_agent_commit.png"

ADD_RES=$(python3 "$IPC_PY" agent-add-track "$API_SOCK" 28.0 94.0 155.0 132.0 2.8 3)
sleep 0.7
import -window root "$OUT/shots/01b_after_agent_commit.png"

N1=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
MOD1=$(is_doc_modified "$API_SOCK" pcb "$PCB_XID")
AGENT_DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/01a_before_agent_commit.png" "$OUT/shots/01b_after_agent_commit.png" $((BX + BW / 3)) $((BY + 50)) $((BW * 2 / 3 - 50)) $((BH - 100)))

LIVE_OK=$(python3 -c '
import json, sys
n0, n1, mod1 = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
diff = json.loads(sys.argv[4])
print(int(n1 == n0 + 1 and mod1 == 1 and diff["changed_pixels"] > 40))
' "$N0" "$N1" "$MOD1" "$AGENT_DIFF")
check "Live agent IPC commit incremented PCB traces ($N0 -> $N1) and visibly repainted embedded GAL canvas ($AGENT_DIFF)" "$LIVE_OK" "$AGENT_DIFF"

# Human presses Ctrl+Z inside embedded PCB Editor (focused via real click above) to undo agent's staged commit
python3 "$XINPUT_PY" chord ctrl z
sleep 0.6
shot "02_after_human_undo"

N_UNDO=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
check "Human Ctrl+Z inside embedded PCB Editor atomically undid agent's IPC commit ($N1 -> $N_UNDO)" \
  "$([ "$N_UNDO" -eq "$N0" ] && echo 1 || echo 0)" \
  "expected $N0 got $N_UNDO"

# 3. Child freeze (SIGSTOP) isolation + Backplane's OWN passive grab_escape (Ctrl+`) host rescue
python3 "$IPC_PY" agent-add-track "$API_SOCK" 35.0 98.0 140.0 126.0 2.0 3 >/dev/null
sleep 0.35
python3 "$XINPUT_PY" click "$GAL_X" "$GAL_Y"
sleep 0.35

FOC_BEFORE=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT1_XID" "$PCB_XID" | python3 -c 'import json,sys; print(int(bool(json.load(sys.stdin).get("focus_in_child"))))')

kill -STOP "$KP"
T0_MS=$(python3 -c 'import time; print(int(time.time()*1000))')
python3 "$XINPUT_PY" chord ctrl grave
sleep 0.25
FOC_AFTER=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT1_XID" "$PCB_XID" | python3 -c 'import json,sys; print(int(bool(json.load(sys.stdin).get("focus_in_child"))))')
T1_MS=$(python3 -c 'import time; print(int(time.time()*1000))')
ELAPSED_MS=$((T1_MS - T0_MS))

HUB_ALIVE=0
curl -s -m 1 -o /dev/null "http://127.0.0.1:$HPORT/hello" && HUB_ALIVE=1
kill -CONT "$KP"

PING_AFTER_CONT=0
wait_for_ipc "$API_SOCK" 30 && PING_AFTER_CONT=1

FREEZE_OK=0
if [ "$FOC_BEFORE" -eq 1 ] && [ "$FOC_AFTER" -eq 0 ] && [ "$ELAPSED_MS" -lt 1500 ] && [ "$HUB_ALIVE" -eq 1 ] && [ "$PING_AFTER_CONT" -eq 1 ]; then
  FREEZE_OK=1
fi
check "While KiCad is SIGSTOP-frozen, Backplane passive grab_escape (Ctrl+\`) returns focus to GPUI in ${ELAPSED_MS}ms and hub stays responsive" \
  "$FREEZE_OK" "foc=$FOC_BEFORE->$FOC_AFTER elapsed=${ELAPSED_MS}ms hub=$HUB_ALIVE ipc=$PING_AFTER_CONT"

# 4. Dirty-close guard + hard GPUI client SIGKILL survival (XFixes save-set) + warm re-adoption preserving unsaved edits
sleep 0.25
CLOSE_ASK=$(hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"ask\"}}]")
REFUSED_DIRTY=$(python3 - "$CLOSE_ASK" <<'PY'
import json, sys
ok = 0
for line in sys.argv[1].splitlines():
    if not line or line.startswith("INFO "):
        continue
    d = json.loads(line)
    rep = d.get("reply") or d.get("result") or {}
    if d.get("ok") is True and rep.get("closed") is False:
        ok = 1
print(ok)
PY
)
check "editor.kicad.close(save=ask) refuses to close while PCB has unsaved edits" "$REFUSED_DIRTY" "$CLOSE_ASK"

kill -KILL "$CP" 2>/dev/null || true
wait "$CP" 2>/dev/null || true
sleep 0.5

KP_SURVIVED=0
kill -0 "$KP" 2>/dev/null && KP_SURVIVED=1

cat >"$OUT/script2.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":45000,"kind":"PCB","tag":"reattached"}},{"mark":"reattached_ready"}]
EOF

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script2.json" --exit-after 180 >"$OUT/client2.log" 2>&1 &
CP2=$!
track_pid "$CP2"

wait_for_log "$OUT/client2.log" '"name":"reattached_ready","event":"mark"' 240
R_REATT=$(last_kicad_report "$OUT/client2.log" "reattached")
read -r KP_REATT GPUI2_XID CONT2_XID PCB_REATT_XID < <(python3 - "$R_REATT" <<'PY'
import json, sys
r = json.loads(sys.argv[1])
print(
    int(r.get("pid") or 0),
    f"0x{int(r.get('toplevel') or 0):x}",
    f"0x{int((r.get('container') or {}).get('xid') or 0):x}",
    f"0x{int(r.get('shown') or 0):x}",
)
PY
)
sleep 0.5
shot "03_after_container_hotswap"

RPT_SWAP=$(python3 "$DRIVER_PY" report "$GPUI2_XID" "$CONT2_XID" "$PCB_REATT_XID")
MOD_SWAP=$(is_doc_modified "$API_SOCK" pcb "$PCB_REATT_XID")
N_SWAP=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')

SWAP_OK=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1])
surv, kp0, kp1, mod_s, n_s, n0 = map(int, sys.argv[2:8])
print(int(surv == 1 and kp1 == kp0 and bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 20 and mod_s == 1 and n_s == n0 + 1))
' "$RPT_SWAP" "$KP_SURVIVED" "$KP" "$KP_REATT" "$MOD_SWAP" "$N_SWAP" "$N0")
check "KiCad survived GPUI SIGKILL via XFixes save-set and re-embedded into fresh client preserving unsaved edits (pid=$KP_REATT traces=$N_SWAP)" \
  "$SWAP_OK" "surv=$KP_SURVIVED kp=$KP->$KP_REATT mod=$MOD_SWAP traces=$N_SWAP"

# 5. Hard KiCad crash (SIGKILL) recovery via editor.kicad.restart & zero-orphan teardown
kill -KILL "$KP" 2>/dev/null || true
sleep 0.8

GPUI_AFTER_KILL=$(python3 "$DRIVER_PY" find-gpui --client-log "$OUT/client2.log" || echo "")
HUB_AFTER_KILL=0
curl -s -m 1 -o /dev/null "http://127.0.0.1:$HPORT/hello" && HUB_AFTER_KILL=1
check "SIGKILL of embedded KiCad leaves Backplane GPUI window and hub alive without cascading crash" \
  "$([ -n "$GPUI_AFTER_KILL" ] && [ "$HUB_AFTER_KILL" -eq 1 ] && echo 1 || echo 0)"

# `editor.kicad.restart` waits until KiCad finishes startup; in KiCad 10.99-bp.8, LocalHistory's post-crash autosave
# opens a modal "Recover Unsaved Changes" dialog (`transient_for` Backplane's top-level) before `SetReadyToReply`.
# Run `restart` asynchronously so we can observe & answer any crash-recovery dialog just like a human user.
hub_rpc "[{\"method\":\"editor.kicad.restart\",\"timeout_ms\":45000,\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"frame\":\"PCB\"}}]" >"$OUT/restart_rpc.out" 2>&1 &
RESTART_CALL_PID=$!
extract_hub_field() {
  python3 - "$1" "$2" <<'PY'
import json, sys
field = sys.argv[2]
val = ""
for line in sys.argv[1].splitlines():
    if line.startswith("INFO "):
        obj = (json.loads(line[5:]).get("value") or {})
        if obj.get(field):
            val = str(obj[field])
    elif line.startswith("{"):
        obj = (json.loads(line).get("reply") or {})
        if obj.get(field):
            val = str(obj[field])
print(val)
PY
}
KP2=0
for _ in $(seq 60); do
  HINFO=$(hub_rpc "[]")
  KP2=$(extract_hub_field "$HINFO" "pid")
  KP2=${KP2:-0}
  [ "$KP2" -gt 0 ] && [ "$KP2" -ne "$KP" ] && break
  sleep 0.2
done
[ "${KP2:-0}" -gt 0 ] && track_pid "$KP2"

# Poll up to 45s for the restarted PCB Editor to embed & paint, dismissing any post-crash recovery dialog (`Recover Unsaved Changes`) if shown.
PCB2_XID=""
for _ in $(seq 180); do
  REC_DLG=$(python3 "$DRIVER_PY" list-windows --pid "$KP2" 2>/dev/null | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    t = (w.get("title") or "").lower()
    if w.get("shown") and not w.get("override_redirect") and (w.get("transient_for") or "recover" in t or "unsaved" in t or "already open" in t):
        if "pcb editor" not in t and "schematic editor" not in t:
            print(w["xid"]); break
' 2>/dev/null || true)
  if [ -n "$REC_DLG" ]; then
    DLG_INFO=$(python3 "$DRIVER_PY" get-transient "$REC_DLG" 2>/dev/null || echo "{}")
    read -r DX DY DW DH < <(python3 -c 'import json,sys; g=json.loads(sys.argv[1]).get("dialog",{}); print(g.get("root_x",600), g.get("root_y",300), g.get("w",400), g.get("h",180))' "$DLG_INFO")
    python3 "$XINPUT_PY" click $((DX + DW / 2)) $((DY + 20))
    sleep 0.15
    python3 "$XINPUT_PY" key Return
    sleep 0.35
    STILL_UP=$(python3 "$DRIVER_PY" get-transient "$REC_DLG" 2>/dev/null | python3 -c 'import json,sys; print(int(bool(json.load(sys.stdin).get("dialog",{}).get("shown"))))' 2>/dev/null || echo 0)
    if [ "$STILL_UP" = "1" ]; then
      python3 "$XINPUT_PY" key Tab
      sleep 0.1
      python3 "$XINPUT_PY" key Return
      sleep 0.35
    fi
    STILL_UP=$(python3 "$DRIVER_PY" get-transient "$REC_DLG" 2>/dev/null | python3 -c 'import json,sys; print(int(bool(json.load(sys.stdin).get("dialog",{}).get("shown"))))' 2>/dev/null || echo 0)
    if [ "$STILL_UP" = "1" ]; then
      python3 "$XINPUT_PY" click $((DX + DW - 65)) $((DY + DH - 25))
      sleep 0.35
    fi
  fi
  if PCB2_XID=$(wait_for_embedded_frame "$GPUI2_XID" "$CONT2_XID" "$KP2" "PCB Editor" 1 2>/dev/null); then
    break
  fi
done
wait "$RESTART_CALL_PID" 2>/dev/null || true
RESTART_RES=$(cat "$OUT/restart_rpc.out" 2>/dev/null || true)
if [ -z "$PCB2_XID" ]; then
  echo "DIAG timeout waiting for restarted PCB Editor (pid=$KP2 cont=$CONT2_XID); RESTART_RES=$RESTART_RES; HINFO=$(hub_rpc '[]')" >&2
  python3 "$DRIVER_PY" list-windows >&2 || true
fi
API2_SOCK=$(extract_hub_field "$RESTART_RES" "sock")
if [ -z "$API2_SOCK" ]; then
  HINFO2=$(hub_rpc "[]")
  API2_SOCK=$(extract_hub_field "$HINFO2" "sock")
fi
REOPEN_IPC=0
[ -n "$API2_SOCK" ] && wait_for_ipc "$API2_SOCK" 80 && REOPEN_IPC=1
sleep 0.6
shot "04_reopened_after_crash"

RPT_REOPEN="{}"
[ -n "$PCB2_XID" ] && RPT_REOPEN=$(python3 "$DRIVER_PY" report "$GPUI2_XID" "$CONT2_XID" "$PCB2_XID")
REOPEN_OK=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1]); ipc = int(sys.argv[2]); kp1 = int(sys.argv[3]); kp2 = int(sys.argv[4])
print(int(kp2 > 0 and kp2 != kp1 and ipc == 1 and bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 20))
' "$RPT_REOPEN" "$REOPEN_IPC" "$KP" "$KP2")
check "editor.kicad.restart spawned fresh KiCad (pid=$KP2 != $KP) and client re-embedded it cleanly" "$REOPEN_OK" "$RPT_REOPEN"

hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" >/dev/null || true
stop_pid "$CP2"
for _ in $(seq 25); do
  kill -0 "$KP2" 2>/dev/null || break
  sleep 0.1
done

CLEAN_OK=1
kill -0 "$KP" 2>/dev/null && CLEAN_OK=0
kill -0 "$KP2" 2>/dev/null && CLEAN_OK=0
check "editor.kicad.close left zero running KiCad processes" "$CLEAN_OK"

finish_suite "embed_agent_resilience_e2e ($EMBED_MODE)"
