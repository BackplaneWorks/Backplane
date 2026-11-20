#!/bin/bash
# Close{CM_SAVE} with a save that cannot be written (the board file read-only): what KiCad answers and when, what is on screen, and
# what is left open and modified (the hub close-wait PR's question: does a failed save ever answer "exiting"?).
#   R1  no "exiting": true while the save cannot be written (the reply, if any, within READ_S)
#   R2  what is on screen meanwhile (KiCad's error box, a modal: the reply waits for it)  [measured, not judged]
#   R3  after the box is dismissed (Return, as the person's OK): the board frame is still open and still modified
#   R4  the board file on disk is unchanged (the agent's track not in it, the file not truncated)
# Fork mode, copies only. One run.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_close_readonly_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; BOARD="$PROJ/ecc83-pp.kicad_pcb"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":4000},{"kicad":{"report":true,"tag":"ready"}},{"mark":"ready"},{"wait":600000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" \
  --script "$OUT/script.json" --exit-after 600 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 800 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock") or "")' "$R")
KPID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid") or 0)' "$R")
[ "$KPID" -gt 0 ] && track_pid "$KPID"

python3 "$IPC_PY" agent-add-track "$SOCK" 100 100 104 100 0.25 3 >"$OUT/track.out" 2>&1 || true
echo "INFO modified before: $(python3 "$IPC_PY" modified "$SOCK" pcb 2>&1 | tail -1)"
SUM0=$(sha256sum "$BOARD" | cut -c1-16); SEG0=$(grep -c "(segment" "$BOARD" || true)
chmod a-w "$BOARD"
READ_S=${READ_S:-20}
T0=$(date +%s%3N)
( timeout "$READ_S" python3 "$IPC_PY" close "$SOCK" save >"$OUT/close.out" 2>&1; echo "rc $? at $(( $(date +%s%3N) - T0 )) ms" >>"$OUT/close.out" ) &
CL=$!
sleep 6
WINS=$(xwininfo -root -tree 2>/dev/null | grep -E '"[^"]+"' | grep -viE "i3|Backplane GPUI" | sed 's/^ *//' | cut -c1-90 | grep -iE "error|save|kicad|pcb" | tr '\n' ';')
echo "INFO on screen 6 s after the Close: $WINS"
echo "INFO focus: $(xfocus_chain)"
wait "$CL"
echo "INFO the Close's reply: $(tr '\n' ' ' <"$OUT/close.out" | cut -c1-300)"
EXITING=$(grep -c '"exiting": true' "$OUT/close.out" || true)
check "R1 no 'exiting': true while the save cannot be written" "$([ "${EXITING:-0}" = 0 ] && echo 1 || echo 0)" "$(head -c 200 "$OUT/close.out")"
# the person's OK on the error box (the focused window's default button), then KiCad asked again
python3 "$XINPUT_PY" key Return >/dev/null 2>&1; sleep 2
WINS2=$(xwininfo -root -tree 2>/dev/null | grep -E '"[^"]+"' | grep -viE "i3|Backplane GPUI" | sed 's/^ *//' | cut -c1-90 | grep -iE "error|save" | tr '\n' ';')
echo "INFO on screen after Return: ${WINS2:-nothing}"
FR=$(timeout 20 python3 "$IPC_PY" frames "$SOCK" 2>&1 | tail -1)
MOD=$(timeout 20 python3 "$IPC_PY" modified "$SOCK" pcb 2>&1 | tail -1)
echo "INFO frames after: $(echo "$FR" | cut -c1-300)"; echo "INFO modified after: $MOD"
check "R3 the board frame is still open after the failed save" "$(echo "$FR" | grep -qiE '"kind": *"?(1|PCB|pcb)' && echo 1 || echo 0)" "$(echo "$FR" | cut -c1-200)"
check "R3 and still modified" "$(echo "$MOD" | grep -q '"modified": true' && echo 1 || echo 0)" "$MOD"
check "R4 the board file on disk is unchanged" "$([ "$(sha256sum "$BOARD" | cut -c1-16)" = "$SUM0" ] && echo 1 || echo 0)" "segments $SEG0 -> $(grep -c '(segment' "$BOARD")"
chmod u+w "$BOARD"
timeout 30 python3 "$IPC_PY" close "$SOCK" discard >"$OUT/close2.out" 2>&1 || true
finish_suite "embed_close_readonly_e2e ($EMBED_MODE)"
