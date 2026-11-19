#!/bin/bash
# An agent's footprint exchange while the person's Choose Symbol has started KiCad's full footprint load (bp.13.2 b5/b6 Fix 1: the
# chooser loads the libraries in the background; that ends KiCad's on-demand loads of single libraries until the load reaches each,
# ~13 s). KiCad answers "not found" for a footprint whose library the load has not reached yet; kicad_edit waits for that library by
# its status (GetLibraryStatuses), each library on its own.
#   L1  Choose Symbol opens in the embedded schematic editor; a symbol is picked out by its search (its footprint selector asks for the
#       footprints: the full load starts), then closed
#   L0  KiCad is held to one CPU, so its footprint load lasts long enough for the edit to land inside it
#   L2  at once, one kicad_edit exchanges C1 for a footprint of Capacitor_THT (11th of the stock table's 155) and U1 for the same valve
#       footprint from Valve (154th, the end of the queue): both are exchanged and the board is saved. Before the fix the second failed: once one footprint had come from a
#       library, a later "not found" was final (or a library's 10.5 s wait ran out). (The board's own footprints say `Footprints:`, a
#       library no table has: not used here.)
#   L2  fails unless the edit landed inside the footprint load and asked for the library statuses at least once (else it proves nothing)
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

# Where KiCad's footprint load of Choose Symbol stands (its ET_PROGRESS in the hub's log, BACKPLANE_KICAD_DEBUG): `state` is none,
# started (fraction -1 seen after the chooser opened) or done (that load's fraction 1); `gap` is hit when the edit's first
# PlaceFootprintFromLibrary came after the load started and before it reached fraction 1, else missed (with the log lines)
fp_load() { python3 - "$OUT/hub.log" "$1" <<'PY'
import json, sys
lines = open(sys.argv[1], errors="replace").read().splitlines(); mode = sys.argv[2]
opened = next((i for i, l in enumerate(lines) if "ET_DIALOG_OPENED" in l and "Choose Symbol" in l), None)
start = end = place = None; pid = None
if opened is not None:
    for i in range(opened, len(lines)):
        l = lines[i]
        if place is None and "request kiapi.board.commands.PlaceFootprintFromLibrary" in l: place = i
        if not l.startswith("kicad embed event: "): continue
        try: p = json.loads(l[len("kicad embed event: "):]).get("progress") or {}
        except Exception: continue
        if p.get("title") != "Loading Footprint Libraries": continue
        if pid is None and p.get("fraction") == -1: pid, start = p.get("id"), i
        elif pid is not None and p.get("id") == pid and end is None and (p.get("fraction") == 1 or p.get("done")): end = i
if mode == "state": print("done" if end is not None else "started" if start is not None else "none")
else: print("hit" if start is not None and place is not None and start < place and (end is None or place < end) else f"missed (load started at line {start}, reached 1 at {end}, first PlaceFootprintFromLibrary at {place})")
PY
}

# L0: KiCad held to one CPU (every thread of it, taskset -a): its footprint load takes long enough for the edit, sent through the
# stand-in agent (~5 s), to land inside it (a 32-core desktop finished it while the chooser was still open; on two CPUs it still
# finished before main's edit arrived)
PIN=$(taskset -a -p -c 0 "$KP" 2>&1 | tail -1 || true)
check "L0 KiCad is held to CPU 0" "$(taskset -c -p "$KP" 2>/dev/null | grep -qE 'list: 0$' && echo 1 || echo 0)" "$PIN"

# L1: the schematic editor, Choose Symbol (A on its canvas), a symbol picked out by its search; closed the moment the search's
# selection has started the footprint load
click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:Schematic"
SCH_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "Schematic Editor" 160)
sleep 1
RPT_SCH=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SCH_XID")
read -r SX SY SW SH < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT_SCH")
python3 "$XINPUT_PY" click $((SX + SW * 2 / 3)) $((SY + SH / 2)); sleep 0.5
python3 "$XINPUT_PY" key a
CHOOSER=$(wait_for_kicad_win "$KP" "Choose Symbol" 120 || true)
check "L1 Choose Symbol opened" "$([ -n "$CHOOSER" ] && echo 1 || echo 0)" "${CHOOSER:-no window}"
sleep 1
for k in n e 5 5 5 p; do python3 "$XINPUT_PY" key "$k"; sleep 0.12; done
for _ in $(seq 300); do [ "$(fp_load state)" != none ] && break; sleep 0.1; done
LOAD=$(fp_load state)
check "L1 the search's selection started KiCad's footprint load (fraction -1)" "$([ "$LOAD" != none ] && echo 1 || echo 0)" "load $LOAD"
python3 "$XINPUT_PY" key Escape; sleep 0.2; python3 "$XINPUT_PY" key Escape

# L2: at once (no settle), two exchanges in one call: a library early in the table, then the one at the end of its queue (Valve, 154th
# of 155; Resistor_THT, 116th, was already in by the time the edit came: no "not found", nothing of the wait shown)
AT_SEND=$(fp_load state)
STATUSES_BEFORE=$(count "request kiapi.common.commands.GetLibraryStatuses" "$OUT/hub.log")
T0=$(date +%s%N)
E=$(ask kicad_edit "{\"path\":\"$PCB\",\"ops\":[{\"set\":\"C1\",\"footprint\":\"Capacitor_THT:C_Disc_D4.7mm_W2.5mm_P5.00mm\"},{\"set\":\"U1\",\"footprint\":\"Valve:Valve_ECC-83-1\"}]}")
T1=$(date +%s%N)
ASKED=$(( $(count "request kiapi.common.commands.GetLibraryStatuses" "$OUT/hub.log") - STATUSES_BEFORE ))
GAP=$(fp_load gap)
echo "INFO kicad_edit after Choose Symbol ($(( (T1 - T0) / 1000000 )) ms; the load was $AT_SEND when it was sent): $(echo "$E" | cut -c1-400)"
check "L2 the edit landed inside the footprint load (the gap was hit: otherwise this run proves nothing)" "$([ "$GAP" = hit ] && echo 1 || echo 0)" "$GAP"
check "L2 the edit waited for a library by its status (asks > 0: otherwise this run proves nothing)" "$([ "$ASKED" -gt 0 ] && echo 1 || echo 0)" "$ASKED status asks"
check "L2 both footprints are exchanged and the board is saved" \
  "$([ "$(echo "$E" | grep -o '"swapped":1' | wc -l)" = 2 ] && echo "$E" | grep -q "\"saved\":\"$PCB\"" && echo 1 || echo 0)" "$(echo "$E" | cut -c1-300)"
check "L2 and nothing said a library did not load or was not found" "$(echo "$E" | grep -qE 'not found|did not load' && echo 0 || echo 1)" "$(echo "$E" | grep -oE '[^"]*(not found|did not load)[^"]*' | head -1)"
check "L2 the saved board has C1 on Capacitor_THT and U1 on Valve" "$(grep -q '(footprint "Capacitor_THT:C_Disc_D4.7mm_W2.5mm_P5.00mm"' "$PCB" && grep -q '(footprint "Valve:Valve_ECC-83-1"' "$PCB" && echo 1 || echo 0)"
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 5 || true
finish_suite "embed_lib_wait_e2e ($EMBED_MODE)"
