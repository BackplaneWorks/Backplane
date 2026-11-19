#!/bin/bash
# An agent's footprint exchange while the person's Choose Symbol has started KiCad's full footprint load (bp.13.2 b5/b6 Fix 1: the
# chooser loads the libraries in the background; that ends KiCad's on-demand loads of single libraries until the load reaches each,
# ~13 s). KiCad answers "not found" for a footprint whose library the load has not reached yet; kicad_edit waits for that library by
# its status (GetLibraryStatuses), each library on its own.
#   L1  Choose Symbol opens in the embedded schematic editor; a symbol is picked out by its search (its footprint selector asks for the
#       footprints: the full load starts), then closed
#   L2  at once, one kicad_edit exchanges C1 for a footprint of Capacitor_THT (early in the stock table) and R3 for one of Resistor_THT
#       (late in it): both are exchanged and the board is saved. Before the fix the second failed: once one footprint had come from a
#       library, a later "not found" was final (or a library's 10.5 s wait ran out). (The board's own footprints say `Footprints:`, a
#       library no table has: not used here.)
#   INFO whether the gap was hit: how often the edit asked KiCad for the library statuses (0: the load had already reached Resistor_THT)
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_lib_wait_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000 BACKPLANE_KICAD_DEBUG=1
start_hub

# how many lines of $2 match $1 (0 when none or no file: grep -c prints 0 and fails then, so never `|| echo 0`)
count() { local n; n=$(grep -c "$1" "$2" 2>/dev/null || true); echo "${n:-0}"; }
ask() { local N; N="$1-$(date +%s%N)"; local before; before=$(count "^MCPCALL" "$OUT/standin.log")
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mcl$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 480); do [ "$(count "^MCPCALL" "$OUT/standin.log")" -gt "$before" ] && break; sleep 0.25; done; sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 | sed "s/^MCPCALL $1 //" | python3 -c 'import json,sys
try: o=json.loads(sys.stdin.read()); print(o["result"]["content"][0]["text"].replace("\n"," "))
except Exception as e: print("?")'; }

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":60000,"kind":"PCB","tag":"pcb_init"}},
 {"mark":"host_ready"},{"wait":600000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 720 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"host_ready","event":"mark"' 240 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R_INIT=$(last_kicad_report "$OUT/client.log" "pcb_init")
read -r KP GPUI_XID CONT_XID < <(python3 - "$R_INIT" <<'PY'
import json, sys
r = json.loads(sys.argv[1])
print(int(r.get("pid") or 0), f"0x{int(r.get('toplevel') or 0):x}", f"0x{int((r.get('container') or {}).get('xid') or 0):x}")
PY
)
[ "$KP" -gt 0 ] && track_pid "$KP"

# L1: the schematic editor, Choose Symbol (A on its canvas), a symbol picked out by its search, closed
click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:Schematic"
SCH_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "Schematic Editor" 160)
sleep 1
RPT_SCH=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SCH_XID")
read -r SX SY SW SH < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT_SCH")
python3 "$XINPUT_PY" click $((SX + SW * 2 / 3)) $((SY + SH / 2)); sleep 0.5
python3 "$XINPUT_PY" key a
CHOOSER=$(wait_for_kicad_win "$KP" "Choose Symbol" 40 || true)
check "L1 Choose Symbol opened" "$([ -n "$CHOOSER" ] && echo 1 || echo 0)" "${CHOOSER:-no window}"
sleep 1
for k in n e 5 5 5 p; do python3 "$XINPUT_PY" key "$k"; sleep 0.12; done
sleep 2.5   # the search's first match is selected: its footprint selector asks for the footprints, which starts the full load
shot l1_chooser 2>/dev/null || true
python3 "$XINPUT_PY" key Escape; sleep 0.3; python3 "$XINPUT_PY" key Escape; sleep 0.3

# L2: at once, two exchanges in one call: a library early in the table, then one late in it
STATUSES_BEFORE=$(count "request kiapi.common.commands.GetLibraryStatuses" "$OUT/hub.log")
T0=$(date +%s%N)
E=$(ask kicad_edit "{\"path\":\"$PCB\",\"ops\":[{\"set\":\"C1\",\"footprint\":\"Capacitor_THT:C_Disc_D4.7mm_W2.5mm_P5.00mm\"},{\"set\":\"R3\",\"footprint\":\"Resistor_THT:R_Axial_DIN0207_L6.3mm_D2.5mm_P7.62mm_Horizontal\"}]}")
T1=$(date +%s%N)
ASKED=$(( $(count "request kiapi.common.commands.GetLibraryStatuses" "$OUT/hub.log") - STATUSES_BEFORE ))
echo "INFO kicad_edit after Choose Symbol ($(( (T1 - T0) / 1000000 )) ms): $(echo "$E" | cut -c1-400)"
if [ "$ASKED" -gt 0 ]; then echo "INFO the edit waited for a library: $ASKED status asks (the gap was hit)"
else echo "INFO no status asks: the full load had already reached Resistor_THT (the gap was not hit; this run shows nothing of the wait)"; fi
check "L2 both footprints are exchanged and the board is saved" \
  "$([ "$(echo "$E" | grep -o '"swapped":1' | wc -l)" = 2 ] && echo "$E" | grep -q "\"saved\":\"$PCB\"" && echo 1 || echo 0)" "$(echo "$E" | cut -c1-300)"
check "L2 and nothing said a library did not load or was not found" "$(echo "$E" | grep -qE 'not found|did not load' && echo 0 || echo 1)" "$(echo "$E" | grep -oE '[^"]*(not found|did not load)[^"]*' | head -1)"
check "L2 the saved board has C1 on Capacitor_THT and R3 on Resistor_THT" "$(grep -q '(footprint "Capacitor_THT:C_Disc_D4.7mm_W2.5mm_P5.00mm"' "$PCB" && grep -q '(footprint "Resistor_THT:R_Axial_DIN0207_L6.3mm_D2.5mm_P7.62mm_Horizontal"' "$PCB" && echo 1 || echo 0)"
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 5 || true
finish_suite "embed_lib_wait_e2e ($EMBED_MODE)"
