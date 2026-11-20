#!/bin/bash
# Measurement for the hub close-wait PR (the KiCad lead's "never kill while saving or asking", before code): the person's Quit from KiCad's
# menu with the board and the schematic modified. A timeline in ms from the Quit of when KiCad's API socket stops answering, when the
# "Save changes?" question is on screen and goes, when each file is written, and when the process ends; the question is left unanswered
# QUIT_ASK_S seconds (30), then answered with its default button (Return: Save). The files are checked after the exit: loadable
# (kicad-cli), and the agent's track and wire in them. Single run; nothing judged but the save (the timeline is the result).
#   QUIT_ASK_S=30   how long the question is left before it is answered
#   QUIT_KEY=Return the key that answers it (Return: its default)
#   QUIT_VIA=closesave  not the menu's Quit: Close{CM_SAVE} on KiCad's socket (the hub's own save-close), its answer's time in the output
# Copies only (seed_project.py), fork mode.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_quit_save_measure: fork only"; exit 0; }
ASK=${QUIT_ASK_S:-30}
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"; PCB="$PROJ/ecc83-pp.kicad_pcb"; SCH="$PROJ/ecc83-pp.kicad_sch"
export BACKPLANE_KICAD_DEBUG=1
start_xvfb_and_wm; start_hub
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":5000},{"kicad":{"report":true,"tag":"ready"}},{"mark":"ready"},{"wait":600000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 400 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 900 || { echo "FAIL the editor did not come"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock") or "")' "$R"); KPID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid") or 0)' "$R")
SEG0=$(grep -c "(segment" "$PCB" || true); WIRE0=$(grep -c "(wire" "$SCH" || true)
python3 "$IPC_PY" agent-add-track "$SOCK" 100 100 104 100 0.25 3 >"$OUT/track.out" 2>&1 || true
python3 "$IPC_PY" agent-add-wire "$SOCK" 50 50 60 50 >"$OUT/wire.out" 2>&1 || true
MODP=$(python3 "$IPC_PY" modified "$SOCK" pcb 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?")
MODS=$(python3 "$IPC_PY" modified "$SOCK" sch 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?")
echo "INFO KiCad $KPID on $SOCK; modified: board $MODP, schematic $MODS; board segments $SEG0, schematic wires $WIRE0"
# the sampler starts first, then the Quit (KiCad's menu action, as the person's): t=0 is the sampler's start, the Quit ~0.3 s after
python3 "$KEMBED_LIB/quit_sampler.py" --sock "$SOCK" --pid "$KPID" --ipc "$IPC_PY" --driver "$DRIVER_PY" --xinput "$XINPUT_PY" --hub-log "$OUT/hub.log" \
  --file "$PCB" --file "$SCH" --answer-at $((ASK + 1)) --answer-title "save" --answer-key "${QUIT_KEY:-Return}" --max $((ASK + 60)) >"$OUT/timeline.jsonl" 2>&1 &
SP=$!
sleep 0.3; echo "{\"quit_at_epoch_ms\": $(date +%s%3N)}" >"$OUT/quit.json"
if [ "${QUIT_VIA:-menu}" = closesave ]; then
  # the hub's own Close{CM_SAVE} as KiCad sees it (sent here straight to the socket: the hub's 800 ms kill would end it early): when it
  # answers "exiting" against the saves, Local History and the exit in the timeline
  ( A=$(date +%s%3N); KIPC_TIMEOUT=90 timeout 100 python3 "$IPC_PY" close "$SOCK" save >"$OUT/quit.out" 2>&1; echo "rc=$? asked_epoch_ms=$A answered_epoch_ms=$(date +%s%3N)" >>"$OUT/quit.out" ) &
else
( timeout $((ASK + 30)) python3 "$IPC_PY" run-action "$SOCK" common.Control.quit >"$OUT/quit.out" 2>&1; echo "rc=$?" >>"$OUT/quit.out" ) &
fi
wait "$SP"
echo "INFO the Quit's own answer: $(tr '\n' ' ' <"$OUT/quit.out" | cut -c1-200)"
echo "INFO timeline (ms from the sampler's start; the Quit at ~300):"; sed 's/^/INFO   /' "$OUT/timeline.jsonl" | cut -c1-260
SEG1=$(grep -c "(segment" "$PCB" || true); WIRE1=$(grep -c "(wire" "$SCH" || true)
"${KICAD_CLI:-kicad-cli}" pcb export svg --output "$OUT/pcb.svg" --layers F.Cu "$PCB" >"$OUT/cli-pcb.out" 2>&1; PCBOK=$?
"${KICAD_CLI:-kicad-cli}" sch export svg --output "$OUT/sch-svg" "$SCH" >"$OUT/cli-sch.out" 2>&1; SCHOK=$?
echo "INFO after: board segments $SEG0 -> $SEG1 (kicad-cli $PCBOK), schematic wires $WIRE0 -> $WIRE1 (kicad-cli $SCHOK); KiCad alive: $(kill -0 "$KPID" 2>/dev/null && echo yes || echo no)"
KILLS=$(grep -c "a hang after .*pid $KPID" "$OUT/hub.log" || true)
SESS=$(hub_rpc "[]" 2>/dev/null | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: v=json.load(sys.stdin).get("value") or {}
except Exception: v={}
if isinstance(v,str): v=json.loads(v)
print((v.get("runtime") or {}).get("session",""))' 2>/dev/null || true)
ASKED=$(grep -c '"answered"' "$OUT/timeline.jsonl" || true); ALIVE_AT_ASK=$(python3 -c 'import json,sys
alive=None; first=None
for l in open(sys.argv[1]):
    j=json.loads(l)
    if "alive" in j: alive=j["alive"]
    if "answered" in j and first is None: first=alive
print(int(bool(first)))' "$OUT/timeline.jsonl")
echo "INFO the question answered $ASKED time(s); KiCad alive when the first was answered: $ALIVE_AT_ASK; hub kills $KILLS; the session after: $SESS"
check "b the person's Quit with unsaved changes, the question left up ${ASK} s: KiCad never killed, alive until answered, recorded as closed by the person" "$([ "$ALIVE_AT_ASK" = 1 ] && [ "${KILLS:-0}" = 0 ] && [ "$SESS" = closedByPerson ] && echo 1 || echo 0)" "alive $ALIVE_AT_ASK kills $KILLS session $SESS"
check "the save after the question was answered is complete: both files load, the track and the wire are in them" "$([ "$PCBOK" = 0 ] && [ "$SCHOK" = 0 ] && [ "$SEG1" -gt "$SEG0" ] && [ "$WIRE1" -gt "$WIRE0" ] && echo 1 || echo 0)" "segments $SEG0->$SEG1 wires $WIRE0->$WIRE1 cli $PCBOK/$SCHOK"
kill "$CP" 2>/dev/null; wait "$CP" 2>/dev/null || true
finish_suite "embed_quit_save_measure ($EMBED_MODE)"
