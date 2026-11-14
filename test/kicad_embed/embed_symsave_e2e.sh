#!/bin/bash
# An open, empty Symbol Editor must not take other documents' saves (upstream KiCad keeps its API handlers in pointer order and the Symbol
# Editor's SaveDocument refused before checking the document was its own: "the open symbol has no library yet"; fork fix 93d03b6527, bp.12.4).
# The person takes the editor (Edit), the hub opens KiCad's Symbol Editor (editor.kicad.show SYMBOL_EDITOR, no symbol), then:
# (1) the hub's Board Setup press (fab.profile apply) saves, read back from the project file; (2) an agent, in one turn, takes the board's
# lock, adds a track (kicad_ipc CreateItems) and saves it (SaveDocument), read back from the board file. On a runtime without the fix
# either may be refused: then the hub's words must name the Symbol Editor (savegate::explain_save_error) and the suite fails. fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_symsave_e2e: fork only"; exit 0; }
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' -not -name '*mic_flex*' | sort | head -1); PCB=${PRO%.kicad_pro}.kicad_pcb
# a Board Setup suggestion to press: the clearance minimum under the fab's
python3 - "$PRO" <<'PY'
import json, sys
p = sys.argv[1]; j = json.load(open(p)); j.setdefault("board", {}).setdefault("design_settings", {}).setdefault("rules", {})["min_clearance"] = 0.0
json.dump(j, open(p, "w"), indent=2)
PY
git -C "$OUT/proj" commit -qam "clearance under the fab's" || true
clearance() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["board"]["design_settings"]["rules"].get("min_clearance"))' "$PRO"; }
segs() { grep -c '^\s*(segment' "$PCB" || true; }
C0=$(clearance); S0=$(segs)
echo "INFO before: min_clearance $C0, $S0 segments"
start_xvfb_and_wm; start_hub
cat > "$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"edit"}},{"mark":"edit"},{"wait":120000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 1200); do grep -q '"name":"edit","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
echo "INFO KiCad: $(grep -o 'kicad embed: running [^ ]*' "$OUT/hub.log" | tail -1 || true)"
# KiCad's Symbol Editor opened by the hub, as the person's Symbol Editor button does: open, nothing loaded
SH=$(hub_rpc "[{\"method\":\"editor.kicad.show\",\"params\":{\"project\":\"$PRO\",\"frame\":\"SYMBOL_EDITOR\"}}]" 2>&1 | grep -a '^{"m":"editor.kicad.show"' | head -1 || true)
echo "$SH" > "$OUT/show.json"
# its window shown (the hub's answer alone does not say KiCad made the frame)
SYMW=0; for _ in $(seq 60); do
  SYMW=$(python3 "$DRIVER_PY" list-windows --match "Symbol Editor" 2>/dev/null | python3 -c 'import json,sys; print(sum(1 for w in json.load(sys.stdin) if w.get("shown") and w.get("w",0) >= 180))' 2>/dev/null || echo 0)
  [ "${SYMW:-0}" -ge 1 ] && break; sleep 0.5; done
python3 "$DRIVER_PY" list-windows 2>/dev/null | python3 -c 'import json,sys; print("INFO windows:", [(w.get("title") or "")[:40] for w in json.load(sys.stdin) if w.get("shown")][:12])' 2>/dev/null || true
sleep 2
check "KiCad's Symbol Editor is open (its window shown)" "$([ "${SYMW:-0}" -ge 1 ] && echo 1 || echo 0)" "$(python3 -c 'import json,sys; r=json.load(open(sys.argv[1]))["result"]; print({k: r.get(k) for k in ("ok","state","frame","error","say")})' "$OUT/show.json" 2>/dev/null)"
# (1) the Board Setup press
AP=$(hub_rpc "[{\"method\":\"fab.profile\",\"params\":{\"pro\":\"$PRO\",\"action\":\"apply\",\"key\":\"min_clearance\"}}]" 2>&1 | grep -a '^{"m":"fab.profile"' | head -1 || true)
echo "INFO Board Setup press: $(echo "$AP" | cut -c1-500)"
sleep 2
C1=$(clearance)
SAVED=$(echo "$AP" | python3 -c 'import json,sys
try: print(json.loads(sys.stdin.read())["result"].get("saved"))
except Exception: print("?")')
check "Symbol Editor open: the Board Setup press saves" "$([ "$SAVED" = True ] && echo 1 || echo 0)" "$(echo "$AP" | grep -o '"not_saved":"[^"]*' | cut -c1-300 || true)"
check "the saved project file has the new clearance" "$([ "$C1" != "$C0" ] && echo 1 || echo 0)" "$C0 -> $C1"
# (2) an agent: the board's lock (named in the hub's refusal of a lockless write), then in one turn the lock, a track, the save
ask() {  # ask TEXT: one stand-in turn; waits for N new MCPCALL lines
  local want=$2 before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true); before=${before:-0}
  local text; text=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"msym$(date +%s%N)\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 360); do n=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true); [ "${n:-0}" -ge $((before + want)) ] && break; sleep 0.25; done
  sleep 1.5
}
PCBF=$(basename "$PCB")
DOC="{\"type\":\"DOCTYPE_PCB\",\"board_filename\":\"$PCBF\"}"
ask "mcpcall:{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.SaveDocument\",\"message\":{\"document\":$DOC}}}" 1
LOCK=$(grep "^MCPCALL kicad_ipc" "$OUT/standin.log" | tail -1 | grep -o 'lock_acquire kicad-[A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2 || true)
echo "INFO the board's lock: ${LOCK:-none named}"
TRACK="{\"@type\":\"type.googleapis.com/kiapi.board.types.Track\",\"start\":{\"x_nm\":\"60000000\",\"y_nm\":\"60000000\"},\"end\":{\"x_nm\":\"70000000\",\"y_nm\":\"60000000\"},\"width\":{\"value_nm\":\"300000\"},\"layer\":\"BL_F_Cu\"}"
SEQ="[{\"name\":\"lock_acquire\",\"arguments\":{\"name\":\"$LOCK\"}},{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":$DOC},\"items\":[$TRACK]}}},{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.SaveDocument\",\"message\":{\"document\":$DOC}}}]"
[ -n "$LOCK" ] || SEQ="[${SEQ#*\},}"  # no lock named (none needed): the track and the save only
ask "mcpseq:$SEQ" "$(python3 -c 'import json,sys; print(len(json.loads(sys.argv[1])))' "$SEQ")"
grep "^MCPCALL" "$OUT/standin.log" | tail -3 | cut -c1-400 | sed 's/^/INFO /'
SV=$(grep "^MCPCALL kicad_ipc" "$OUT/standin.log" | tail -1)
CR=$(grep "^MCPCALL kicad_ipc" "$OUT/standin.log" | tail -2 | head -1)
sleep 1
S1=$(segs)
check "the agent's track was made" "$(echo "$CR" | grep -q 'isError\":true\|AS_BAD\|refused' && echo 0 || echo 1)" "$(echo "$CR" | cut -c1-300)"
check "Symbol Editor open: the agent's SaveDocument on the board succeeds" "$(echo "$SV" | grep -q 'isError\":true\|no library yet' && echo 0 || echo 1)" "$(echo "$SV" | cut -c1-400)"
check "the saved board file has the agent's track" "$([ "$S1" -gt "$S0" ] && echo 1 || echo 0)" "$S0 -> $S1 segments"
# a refusal, if any, said in the hub's words (the runtime without the fix)
if grep -q 'no library yet' "$OUT/standin.log" <(echo "$AP"); then
  check "a refusal names the Symbol Editor and what to do" "$(grep -h 'no library yet' "$OUT/standin.log" <(echo "$AP") | grep -q "Close KiCad's Symbol Editor" && echo 1 || echo 0)"
fi
kill "$CP" 2>/dev/null || true
finish_suite "embed_symsave_e2e ($EMBED_MODE)"
