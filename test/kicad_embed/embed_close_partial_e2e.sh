#!/bin/bash
# The host's whole-KiCad Close (the hub's), with the board and the schematic both modified by an agent (the KiCad lead's two questions on
# the fork's no-modals Close, next batch):
#   CLOSE_CASE=discard  Close{CM_DISCARD}: KiCad ends (exiting), nothing is saved (both files unchanged), nothing asked
#     D1 the answer is exiting   D2 the board and the schematic files are unchanged   D3 KiCad's process ends by itself
#   CLOSE_CASE=partial  the board read-only, Close{CM_SAVE}: a save fails part way; what KiCad is left as, and that it is usable
#     P1 the answer is an error, not exiting   P2 which frames are left open, which files were written (measured)
#     P3 every editor shows again (ShowFrame), an agent edit lands, and a Close{CM_DISCARD} then ends it
# Fork mode, copies only. One run.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_close_partial_e2e: fork only"; exit 0; }
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
PCB="$PROJ/ecc83-pp.kicad_pcb"; SCH="$PROJ/ecc83-pp.kicad_sch"
python3 "$IPC_PY" show-frame "$SOCK" sch >"$OUT/show-sch.out" 2>&1 || true; sleep 3
python3 "$IPC_PY" agent-add-track "$SOCK" 100 100 104 100 0.25 3 >"$OUT/track.out" 2>&1 || true
python3 "$IPC_PY" agent-add-wire "$SOCK" 50 50 60 50 >"$OUT/wire.out" 2>&1 || true
echo "INFO modified before: board $(python3 "$IPC_PY" modified "$SOCK" pcb 2>&1 | tail -1), schematic $(python3 "$IPC_PY" modified "$SOCK" sch 2>&1 | tail -1)"
echo "INFO frames before: $(python3 "$IPC_PY" frames "$SOCK" 2>&1 | python3 -c 'import json,sys; print([f["kind"] for f in json.load(sys.stdin)["frames"]])' 2>&1)"
P0=$(sha256sum "$PCB" | cut -c1-16); S0=$(sha256sum "$SCH" | cut -c1-16)
CASE=${CLOSE_CASE:-discard}
if [ "$CASE" = discard ]; then
  KIPC_TIMEOUT=20 timeout 25 python3 "$IPC_PY" close "$SOCK" discard >"$OUT/close.out" 2>&1
  echo "INFO the Close's reply: $(tr '\n' ' ' <"$OUT/close.out" | cut -c1-240)"
  check "D1 the answer is exiting" "$(grep -q '"exiting": true' "$OUT/close.out" && echo 1 || echo 0)" "$(head -c 200 "$OUT/close.out")"
  for _ in $(seq 120); do kill -0 "$KPID" 2>/dev/null || break; sleep 0.25; done
  check "D2 the board and the schematic files are unchanged (nothing saved)" "$([ "$(sha256sum "$PCB" | cut -c1-16)" = "$P0" ] && [ "$(sha256sum "$SCH" | cut -c1-16)" = "$S0" ] && echo 1 || echo 0)"
  check "D3 KiCad's process ends by itself" "$(kill -0 "$KPID" 2>/dev/null && echo 0 || echo 1)"
  ls -a "$PROJ" | grep -iE "autosave|^~|lck" | sed "s/^/INFO left in the project: /"
else
  chmod a-w "$PCB"
  KIPC_TIMEOUT=20 timeout 25 python3 "$IPC_PY" close "$SOCK" save >"$OUT/close.out" 2>&1
  echo "INFO the Close's reply: $(tr '\n' ' ' <"$OUT/close.out" | cut -c1-300)"
  check "P1 the answer is an error, not exiting" "$(grep -q '"exiting": true' "$OUT/close.out" && echo 0 || echo 1)" "$(head -c 200 "$OUT/close.out")"
  FR=$(timeout 20 python3 "$IPC_PY" frames "$SOCK" 2>&1 | tail -1)
  echo "INFO P2 frames left open: $(echo "$FR" | python3 -c 'import json,sys; print([(f["kind"], f["title"]) for f in json.load(sys.stdin)["frames"]])' 2>&1)"
  echo "INFO P2 board file: $([ "$(sha256sum "$PCB" | cut -c1-16)" = "$P0" ] && echo unchanged || echo WRITTEN); schematic file: $([ "$(sha256sum "$SCH" | cut -c1-16)" = "$S0" ] && echo unchanged || echo written)"
  echo "INFO P2 modified now: board $(timeout 20 python3 "$IPC_PY" modified "$SOCK" pcb 2>&1 | tail -1), schematic $(timeout 20 python3 "$IPC_PY" modified "$SOCK" sch 2>&1 | tail -1)"
  echo "INFO P2 the hub's session: $(hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys; v=json.load(sys.stdin).get("value") or {}; print(v.get("state"), [f.get("kind") for f in v.get("frames") or []])' 2>&1)"
  chmod u+w "$PCB"
  OK=1
  for k in pcb sch; do timeout 30 python3 "$IPC_PY" show-frame "$SOCK" $k >"$OUT/show-$k-after.out" 2>&1 || OK=0; grep -q '"ok": true' "$OUT/show-$k-after.out" || OK=0; done
  sleep 2
  python3 "$IPC_PY" agent-add-track "$SOCK" 110 110 114 110 0.25 3 >"$OUT/track2.out" 2>&1 || OK=0
  check "P3 every editor shows again and an agent edit lands" "$OK" "$(cat "$OUT"/show-*-after.out "$OUT/track2.out" 2>/dev/null | tr '\n' ' ' | cut -c1-240)"
  KIPC_TIMEOUT=20 timeout 25 python3 "$IPC_PY" close "$SOCK" discard >"$OUT/close2.out" 2>&1
  for _ in $(seq 120); do kill -0 "$KPID" 2>/dev/null || break; sleep 0.25; done
  check "P3 then a Close{CM_DISCARD} ends it" "$(grep -q '"exiting": true' "$OUT/close2.out" && ! kill -0 "$KPID" 2>/dev/null && echo 1 || echo 0)" "$(head -c 150 "$OUT/close2.out")"
fi
finish_suite "embed_close_partial_e2e ($EMBED_MODE, $CASE)"
