#!/bin/bash
# Issue #1224: a zone fill an agent's own kicad_edit asks for is the agent's, never the person's "Fill Zone(s)"; a fill the person starts
# is theirs. Board ecc83-pp (one GND pour on B.Cu, 122.5..172 x 91.3..135.9 mm). Each agent track is a netless B.Cu track inside the pour,
# so every refill changes the pour.
#   F0  headless (no editor open yet): the agent's saving kicad_edit is served headless and its saved file has the pour refilled around
#       the track (KiCad fills the empty list within the request there: left as it was)
#   F1  embedded: the agent's saving kicad_edit is saved, served by the editor's KiCad, and its answer carries no `person` note
#   F2  the saved file has the pour refilled around the new track (the fill landed before the save), and KiCad says the board unmodified
#       a few seconds later (no fill landed after it)
#   F3  the agent's next writing kicad_edit is not held as if the person were at work
#   F4  a Fill the person starts by hand (B in the editor, after an agent's unfilled track) is theirs: the next writing kicad_edit is held
#       and cites their "Fill Zone(s)"; where KiCad has nothing to refill (no edit of theirs), their rotate instead (a control: passes before
#       and after the fix)
# Copies only (seed_project.py), fork mode, a runtime with `coedit`.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_fill_actor_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
grep -q '"coedit"' "$(dirname "$(dirname "$KICAD_BIN")")/manifest.json" 2>/dev/null || { echo "embed_fill_actor_e2e: the runtime has no coedit (no attributed edits): nothing to check"; exit 0; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub

ask() { local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mcl$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 480); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done; sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 | sed "s/^MCPCALL $1 //" | python3 -c 'import json,sys
try: o=json.loads(sys.stdin.read()); print(o["result"]["content"][0]["text"].replace("\n"," "))
except Exception as e: print("?")'; }
# an agent's saving kicad_edit: one netless B.Cu track at y mm, x 130..160
edit_track() { ask kicad_edit "{\"path\":\"$PCB\",\"ops\":[{\"add\":{\"track\":{\"from\":[130,$1],\"to\":[160,$1],\"width\":0.3,\"layer\":\"B.Cu\"}}}]${2:-}}"; }
# the pour's fill in the saved file (its filled_polygon blocks), as a digest
fills() { python3 - "$PCB" <<'PY'
import hashlib, re, sys
t = open(sys.argv[1]).read(); i = t.index("(zone"); z = t[i:]
print(hashlib.sha1("".join(re.findall(r"\(filled_polygon.*?\n\t\t\)", z, re.S)).encode()).hexdigest()[:12])
PY
}
F_START=$(fills)

# F0: headless (the editor not open yet)
H=$(edit_track 133); echo "INFO headless kicad_edit: $(echo "$H" | cut -c1-300)"
F_H=$(fills)
check "F0 headless: the agent's edit is saved by the headless KiCad" "$(echo "$H" | grep -q '"served_by":"headless"' && echo "$H" | grep -q "\"saved\":\"$PCB\"" && echo 1 || echo 0)" "$(echo "$H" | cut -c1-200)"
check "F0 headless: the saved pour is refilled around the track" "$([ "$F_H" != "$F_START" ] && echo 1 || echo 0)" "fills $F_START -> $F_H"

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},
 {"wait":600000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 720 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 240 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
read -r SOCK FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("sock") or "-", f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
mod() { python3 "$IPC_PY" modified "$SOCK" pcb 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?"; }
python3 "$XINPUT_PY" click 400 500; sleep 0.8   # the person is in the chat, not KiCad
sleep 3

# F1..F3: the embedded editor
A=$(edit_track 131); echo "INFO embedded kicad_edit: $(echo "$A" | cut -c1-400)"
F_A=$(fills); sleep 4; MA=$(mod)
check "F1 embedded: the agent's edit is saved by the editor's KiCad" "$(echo "$A" | grep -q '"served_by":"embedded"' && echo "$A" | grep -q "\"saved\":\"$PCB\"" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "F1 and its answer has no person note" "$(echo "$A" | grep -q '"person":' && echo 0 || echo 1)" "$(echo "$A" | grep -o '"person":.*' | cut -c1-300)"
check "F2 the saved pour is refilled around the new track (the fill came before the save)" "$([ "$F_A" != "$F_H" ] && echo 1 || echo 0)" "fills $F_H -> $F_A"
check "F2 and KiCad says the board unmodified after it (no fill landed after the save)" "$([ "$MA" = False ] && echo 1 || echo 0)" "modified $MA"
A2=$(edit_track 127); echo "INFO next embedded kicad_edit: $(echo "$A2" | cut -c1-300)"
check "F3 the agent's next writing edit is not held for the person" "$(echo "$A2" | grep -q 'The person is working in this KiCad' && echo 0 || echo 1)" "$(echo "$A2" | cut -c1-300)"

# F4: an agent's track with no refill (kicad_ipc, unsaved), then the person fills by hand (a click on empty canvas, B)
B=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.BeginCommit\",\"message\":{\"header\":{\"document\":\"\$doc\"}}}")
CID=$(echo "$B" | python3 -c 'import json,sys
try: print(json.loads(sys.stdin.read())["message"]["id"]["value"])
except Exception: print("")')
ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":\"\$doc\"},\"items\":[{\"@type\":\"type.googleapis.com/kiapi.board.types.Track\",\"start\":{\"x_nm\":130000000,\"y_nm\":125000000},\"end\":{\"x_nm\":160000000,\"y_nm\":125000000},\"width\":{\"value_nm\":300000},\"layer\":\"BL_B_Cu\"}]}}" >/dev/null
E=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.EndCommit\",\"message\":{\"header\":{\"document\":\"\$doc\"},\"id\":{\"value\":\"$CID\"},\"action\":\"CMA_COMMIT\",\"message\":\"an unfilled track\"},\"save\":false}")
echo "INFO the agent's unfilled track: $(echo "$E" | cut -c1-200)"
sleep 125   # the agent's own recent edits age out of the person's two-minute window before the person acts (nothing of theirs yet)
MX=$((FX + FW / 2)); MY=$((FY + FH / 2))
for _ in 1 2 3 4 5 6; do python3 "$XINPUT_PY" wheel "$MX" "$MY" -1; sleep 0.3; done; sleep 1
python3 "$XINPUT_PY" click $((MX + FW / 5)) $((MY + FH / 5)); sleep 0.8
python3 "$XINPUT_PY" key b; sleep 6; shot f4_person_fill
python3 "$XINPUT_PY" click 400 500; sleep 0.8
P=$(edit_track 121); echo "INFO kicad_edit after the person's fill: $(echo "$P" | cut -c1-400)"
held() { echo "$1" | grep -q 'The person is working in this KiCad'; }
if held "$P" && echo "$P" | grep -q 'Fill Zone'; then BY="their Fill (B)"; else
  # KiCad had nothing to refill (bp.12.4 here): the person's own edit instead, a click on a part at zoom to fit and R
  echo "INFO the person's Fill gave no edit of theirs: a rotate instead"
  python3 "$XINPUT_PY" key Home; sleep 1.5; python3 "$XINPUT_PY" click "$MX" "$MY"; sleep 0.6; python3 "$XINPUT_PY" key r; sleep 1; python3 "$XINPUT_PY" key Escape; sleep 2
  python3 "$XINPUT_PY" click 400 500; sleep 0.8
  P=$(edit_track 119); echo "INFO kicad_edit after the person's rotate: $(echo "$P" | cut -c1-400)"
  BY="their rotate"; held "$P" && echo "$P" | grep -q 'Rotate' || BY=""
fi
check "F4 a real edit by the person is theirs: the next writing edit is held and cites it (${BY:-neither})" "$([ -n "$BY" ] && echo 1 || echo 0)" "$(echo "$P" | cut -c1-300)"
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 5 || true
finish_suite "embed_fill_actor_e2e ($EMBED_MODE)"
