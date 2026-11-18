#!/bin/bash
# Issue #1169: looking around the embedded board editor is not editing it. An agent's change is left unsaved (kicad_ipc, save: false),
# then the person only looks: a middle-drag pan, two wheel zooms, a click on empty canvas, a click on a part and Escape (a selection,
# nothing changed). Real X input, as a person does it. Then:
#   P0  the same looking on an unmodified board leaves KiCad saying unmodified
#   P1  KiCad's own modified flag is what it was before the looking (a pan or a click that marks the document modified is the fork's bug,
#       said here as such: the hub must not paper over it)
#   P2  the agent's next commit (default save) is saved, and the hub does not say it wrote edits of the person's (there were none);
#       a runtime without `coedit` (no attributed edits) is told why it is refused in words that say not to send the edit again
#   P3  each agent track is in the saved board exactly once (nothing resent twice)
#   P4  a real edit by the person (a click on a part, R) still counts: the next agent save says it wrote the person's edits too
#       (co-editing runtimes, docs/kicad-coedit.md), or is refused (others)
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_unsaved_person_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
COEDIT=0; grep -q '"coedit"' "$(dirname "$(dirname "$KICAD_BIN")")/manifest.json" 2>/dev/null && COEDIT=1
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},
 {"wait":240000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 240 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
read -r SOCK FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("sock") or "-", f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
echo "INFO runtime $KICAD_BIN (coedit: $COEDIT); editor frame ${FW}x${FH} at $FX,$FY"

# an agent's call through the hub's MCP tool, thread t1, answered as the stand-in's MCPCALL line
ask() { local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mcl$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 240); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done; sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true; }
# one agent track committed through kicad_ipc (BeginCommit, CreateItems, EndCommit): X0 X1 (mm, at y 100) and EXTRA args of EndCommit
track() {
  local B CID
  # KiCad's automatic check after the person's edit (bp.13: AS_BUSY, a task is running) is waited out, as an agent does
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    B=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.BeginCommit\",\"message\":{\"header\":{\"document\":\"\$doc\"}}}")
    echo "$B" | grep -q 'AS_BUSY' || break; sleep 3
  done
  CID=$(echo "$B" | sed 's/^MCPCALL kicad_ipc //' | python3 -c 'import json,sys
try: o=json.loads(sys.stdin.read()); m=json.loads(o["result"]["content"][0]["text"])["message"]; print(m["id"]["value"])
except Exception as e: print("")')
  ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":\"\$doc\"},\"items\":[{\"@type\":\"type.googleapis.com/kiapi.board.types.Track\",\"start\":{\"x_nm\":${1}000000,\"y_nm\":100000000},\"end\":{\"x_nm\":${2}000000,\"y_nm\":100000000},\"width\":{\"value_nm\":300000},\"layer\":\"BL_F_Cu\"}]}}" >/dev/null
  ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.EndCommit\",\"message\":{\"header\":{\"document\":\"\$doc\"},\"id\":{\"value\":\"$CID\"},\"action\":\"CMA_COMMIT\",\"message\":\"an agent's track\"}${3:-}}"
}
# the tool result's text (the JSON the agent reads)
said() { sed 's/^MCPCALL kicad_ipc //' | python3 -c 'import json,sys
try: o=json.loads(sys.stdin.read()); print(o["result"]["content"][0]["text"].replace("\n"," "))
except Exception as e: print("?")'; }
# the person looks (look TAG): the frame's centre is a part at zoom to fit, and empty canvas a fifth of the frame off it once zoomed far out
mod() { python3 "$IPC_PY" modified "$SOCK" pcb 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?"; }
count() { grep -c "(start $1 100)" "$PCB" || true; }

MX=$((FX + FW / 2)); MY=$((FY + FH / 2))
look() {
python3 "$XINPUT_PY" drag "$MX" "$MY" $((MX + 80)) $((MY + 40)) 2; sleep 1; M1=$(mod)
echo "INFO $1 after a middle-drag pan: KiCad modified $M1"
python3 "$XINPUT_PY" wheel "$MX" "$MY" 1; sleep 0.4; python3 "$XINPUT_PY" wheel "$MX" "$MY" -1; sleep 1; M2=$(mod)
echo "INFO $1 after two wheel zooms: KiCad modified $M2"
for _ in 1 2 3 4 5 6; do python3 "$XINPUT_PY" wheel "$MX" "$MY" -1; sleep 0.3; done; sleep 1
CX=$((MX + FW / 5)); CY=$((MY + FH / 5)); shot "$1_far"
python3 "$XINPUT_PY" click "$CX" "$CY"; sleep 1; M3=$(mod)
python3 "$XINPUT_PY" key Home; sleep 1.5          # back to zoom to fit, so the centre is a part again
echo "INFO $1 after a click on empty canvas ($CX,$CY): KiCad modified $M3"
python3 "$XINPUT_PY" click "$MX" "$MY"; sleep 1
python3 "$XINPUT_PY" key Escape; sleep 1; M4=$(mod)
echo "INFO $1 after a click on a part and Escape: KiCad modified $M4"
}
# P0: the same looking with nothing modified: KiCad's flag stays unmodified (a pan or a click that marks the document modified is the
# fork's bug, said here as such: the hub must not paper over it)
B0=$(mod); look clean
check "P0 looking around an unmodified board leaves KiCad saying unmodified" "$([ "$B0" = False ] && [ "$M1" = False ] && [ "$M2" = False ] && [ "$M3" = False ] && [ "$M4" = False ] && echo 1 || echo 0)" "before $B0; pan $M1, zoom $M2, click $M3, select $M4"
python3 "$XINPUT_PY" click 400 500; sleep 0.8   # the person is in the chat, not KiCad, while the agent works
E1=$(track 110 120 ',"save":false'); echo "INFO agent track 1 (save: false): $(echo "$E1" | said | cut -c1-240)"
sleep 2; M0=$(mod); echo "INFO KiCad's modified flag after the agent's unsaved commit: $M0"

look pending; shot p1_looked
check "P1 looking around leaves KiCad's modified flag as it was (pan, zoom, empty click, select)" "$([ "$M1" = "$M0" ] && [ "$M2" = "$M0" ] && [ "$M3" = "$M0" ] && [ "$M4" = "$M0" ] && echo 1 || echo 0)" "before $M0; pan $M1, zoom $M2, click $M3, select $M4 (a change here is KiCad's, a fork bug)"

python3 "$XINPUT_PY" click 400 500; sleep 0.8
E2=$(track 125 135); S2=$(echo "$E2" | said); echo "INFO agent track 2 (default save): $(echo "$S2" | cut -c1-400)"
if [ "$COEDIT" = 1 ]; then
  check "P2 the agent's save after only looking is saved" "$(echo "$S2" | grep -q '"saved":"' && echo 1 || echo 0)" "$(echo "$S2" | cut -c1-300)"
  check "P2 and the hub does not say it wrote edits of the person's" "$(echo "$S2" | grep -q "person's own unsaved edits" && echo 0 || echo 1)" "$(echo "$S2" | grep -o '"save_note":"[^"]*"' || true)"
else
  check "P2 the agent's save after only looking is not refused" "$(echo "$S2" | grep -q 'unsaved edits in KiCad\|refused' && echo 0 || echo 1)" "$(echo "$S2" | cut -c1-300)"
fi
check "P3 each agent track is in the saved board once" "$([ "$(count 110)" = 1 ] && [ "$(count 125)" = 1 ] && echo 1 || echo 0)" "track 1: $(count 110), track 2: $(count 125)"

# a real edit by the person: a click on a part, R (rotate), Escape
python3 "$XINPUT_PY" click "$MX" "$MY"; sleep 0.6; python3 "$XINPUT_PY" key r; sleep 1; python3 "$XINPUT_PY" key Escape; sleep 1.5
M5=$(mod); echo "INFO after the person's rotate: KiCad modified $M5"
python3 "$XINPUT_PY" click 400 500; sleep 0.8
E3=$(track 140 150); S3=$(echo "$E3" | said); echo "INFO agent track 3 after the person's edit: $(echo "$S3" | cut -c1-400)"
if [ "$COEDIT" = 1 ]; then
  check "P4 a real edit by the person still counts: the save says it wrote theirs too" "$(echo "$S3" | grep -q "person's own unsaved edits" && echo 1 || echo 0)" "$(echo "$S3" | cut -c1-300)"
else
  check "P4 a real edit by the person still holds the agent's save" "$(echo "$S3" | grep -q 'unsaved edits' && echo 1 || echo 0)" "$(echo "$S3" | cut -c1-300)"
  check "P4 and the refusal says not to send the edit again" "$(echo "$S3" | grep -qi 'do not send it again\|not send it again' && ! echo "$S3" | grep -q 'then ask again' && echo 1 || echo 0)" "$(echo "$S3" | cut -c1-300)"
fi
shot p2_after
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 10 || true
finish_suite "embed_unsaved_person_e2e ($EMBED_MODE)"
