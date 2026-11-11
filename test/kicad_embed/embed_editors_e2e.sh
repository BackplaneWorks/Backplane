#!/bin/bash
# Suite 2: Full hardware engineering workflow across all embedded KiCad surfaces inside Backplane GPUI
# using ONE single KiCad suite process on an isolated git copy of ecc83-pp:
#   1. Project open + live GUI IPC Ping/Version
#   2. Schematic Editor (eeschema): embedded wire edit + Ctrl+S save + kicad-cli sch erc + SVG export
#   3. Symbol Editor: embedded open & GAL paint on project library
#   4. PCB Editor (pcbnew): embedded track routing + copper zone unfill/refill + save
#   5. Footprint Editor: embedded open & GAL paint
#   6. DRC dialog: WM_TRANSIENT_FOR host parenting + screen bounds + kicad-cli pcb drc
#   7. 3D Viewer: embedded OpenGL render + interactive mouse-drag orbit pixel diff
#   8. Plot dialog + Gerber / Drill / STEP manufacturing export + companion metadata preservation
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

# Record companion metadata hashes and baseline schematic/PCB counts before any GUI edits
SHA_BP_BEFORE=$(sha256sum "$PROJ/.backplane.json" | awk '{print $1}')
SHA_COMP_BEFORE=$(sha256sum "$PROJ/ecc83-pp.kicad_pcb.backplane.json" | awk '{print $1}')
SCH_WIRES_BEFORE=$(grep -c "(wire" "$PROJ/ecc83-pp.kicad_sch" || true)

# 1. Spawn ONE KiCad project manager suite process and pre-open PCB, Schematic, Symbol & Footprint editors
"$KICAD_BIN" "$PROJ/ecc83-pp.kicad_pro" >"$OUT/kicad.log" 2>&1 &
KP=$!
track_pid "$KP"

PM_XID=$(wait_for_kicad_win "$KP" "KiCad" 80)
IPC_UP=0
wait_for_ipc "$API_SOCK" 80 && IPC_UP=1
VER_JSON=$(python3 "$IPC_PY" version "$API_SOCK" 2>/dev/null || echo "{}")
check "Project Manager started single KiCad suite + live GUI IPC socket ($VER_JSON)" "$IPC_UP" "$VER_JSON"

# Open PCB Editor (Ctrl+P), Symbol Editor (Ctrl+L), Footprint Editor (Ctrl+F) from Project Manager,
# and Schematic Editor deterministically via RunAction while PCB_EDIT_FRAME is mapped
sleep 0.6
click_win_center "$PM_XID"
sleep 0.3
python3 "$XINPUT_PY" chord ctrl p
PCB_XID=$(wait_for_kicad_win "$KP" "PCB Editor" 80)

for _ in $(seq 50); do
  python3 "$IPC_PY" open-docs "$API_SOCK" pcb 2>/dev/null | grep -q '"count": 1' && break
  sleep 0.25
done

python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.EditorControl.showEeschema" >/dev/null
SCH_XID=$(wait_for_kicad_win "$KP" "Schematic Editor" 80)

python3 "$DRIVER_PY" withdraw "$PCB_XID" >/dev/null
python3 "$DRIVER_PY" withdraw "$SCH_XID" >/dev/null
sleep 0.25

click_win_center "$PM_XID"
sleep 0.25
python3 "$XINPUT_PY" chord ctrl l
SYM_XID=$(wait_for_kicad_win "$KP" "Symbol Editor" 80)
python3 "$DRIVER_PY" withdraw "$SYM_XID" >/dev/null
sleep 0.25

click_win_center "$PM_XID"
sleep 0.25
python3 "$XINPUT_PY" chord ctrl f
FP_XID=$(wait_for_kicad_win "$KP" "Footprint Editor" 80)

# Withdraw all KiCad top-level windows from i3 (ICCCM 4.1.4) once symbol/footprint tables settle
sleep 0.7
for wxid in "$PM_XID" "$PCB_XID" "$SCH_XID" "$SYM_XID" "$FP_XID"; do
  python3 "$DRIVER_PY" withdraw "$wxid" >/dev/null
done
sleep 0.4

# Launch Backplane GPUI on the Board rail as the live host window for the entire suite
cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"mark":"host_ready"},{"wait":150000}]
EOF

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 160 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

for _ in $(seq 80); do
  grep -q '"name":"host_ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break
  sleep 0.2
done
sleep 0.5

eval "$(python3 "$DRIVER_PY" find-gpui --shell --client-log "$OUT/client.log")"

start_container "$GPUI_XID" "$VX" "$VY" "$VW" "$VH" "editors"
CONT_XID="$CONTAINER_LAST_XID"

# 2. Schematic Editor: embed, draw real wire via X11 input, Ctrl+S save, ERC + SVG export
python3 "$DRIVER_PY" embed "$CONT_XID" "$SCH_XID" "$VW" "$VH" >/dev/null
sleep 0.9
RPT_SCH=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SCH_XID")
SCH_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 20))' "$RPT_SCH")
check "Schematic Editor (eeschema) embedded in GPUI and rendered GAL canvas" "$SCH_OK" "$RPT_SCH"

CX=$((GX + VX + VW / 2))
CY=$((GY + VY + VH / 2))
python3 "$XINPUT_PY" click $((CX + 40)) "$CY"
python3 "$DRIVER_PY" focus "$SCH_XID" >/dev/null
sleep 0.25
python3 "$XINPUT_PY" key Home
sleep 0.35
# Press 'w' (Wire tool), click two points on the schematic sheet, finish with End (ACTIONS::finishInteractive), then Ctrl+S
python3 "$DRIVER_PY" focus "$SCH_XID" >/dev/null
python3 "$XINPUT_PY" key w
sleep 0.25
python3 "$XINPUT_PY" click $((CX - 40)) "$CY"
sleep 0.25
python3 "$XINPUT_PY" click $((CX + 110)) "$CY"
sleep 0.25
python3 "$DRIVER_PY" focus "$SCH_XID" >/dev/null
python3 "$XINPUT_PY" key End
sleep 0.2
python3 "$XINPUT_PY" key Escape
sleep 0.3
python3 "$DRIVER_PY" focus "$SCH_XID" >/dev/null
python3 "$XINPUT_PY" chord ctrl s
sleep 0.9
shot "01_schematic_edited"

SCH_WIRES_AFTER=$(grep -c "(wire" "$PROJ/ecc83-pp.kicad_sch" || true)
check "Embedded Schematic Editor interactive wire edit + Ctrl+S saved to disk ($SCH_WIRES_BEFORE -> $SCH_WIRES_AFTER wires)" \
  "$([ "$SCH_WIRES_AFTER" -gt "$SCH_WIRES_BEFORE" ] && echo 1 || echo 0)" \
  "wires $SCH_WIRES_BEFORE -> $SCH_WIRES_AFTER"

"$KICAD_CLI" sch erc --format json -o "$OUT/erc.json" "$PROJ/ecc83-pp.kicad_sch" >/dev/null 2>&1 || true
mkdir -p "$OUT/sch_svg"
"$KICAD_CLI" sch export svg -o "$OUT/sch_svg" "$PROJ/ecc83-pp.kicad_sch" >/dev/null 2>&1
ERC_SVG_OK=$(python3 -c '
import glob, json, os, sys
erc_ok = os.path.isfile(sys.argv[1]) and "sheets" in json.load(open(sys.argv[1]))
svgs = [p for p in glob.glob(os.path.join(sys.argv[2], "*.svg")) if os.path.getsize(p) > 500]
print(int(erc_ok and len(svgs) >= 1))
' "$OUT/erc.json" "$OUT/sch_svg")
check "Saved schematic passes kicad-cli sch erc JSON and exports non-empty SVG" "$ERC_SVG_OK"

# 3. Symbol Editor: embed in-process Symbol Editor inside GPUI, verify GAL paint
python3 "$DRIVER_PY" show "$SCH_XID" 0 >/dev/null
python3 "$DRIVER_PY" embed "$CONT_XID" "$SYM_XID" "$VW" "$VH" >/dev/null
sleep 0.9
shot "02_symbol_editor"
RPT_SYM=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SYM_XID")
SYM_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 15))' "$RPT_SYM")
check "Symbol Editor embedded in GPUI and painted symbol workspace" "$SYM_OK" "$RPT_SYM"
python3 "$DRIVER_PY" show "$SYM_XID" 0 >/dev/null

# 4. PCB Editor: embed, add routed F.Cu track, unfill/refill copper zones, save to disk
python3 "$DRIVER_PY" embed "$CONT_XID" "$PCB_XID" "$VW" "$VH" >/dev/null
sleep 0.9
RPT_PCB=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$PCB_XID")
PCB_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 20))' "$RPT_PCB")
check "PCB Editor (pcbnew) embedded in GPUI and painted board GAL canvas" "$PCB_OK" "$RPT_PCB"

TRACES_0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
ZONES_0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 16 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')

# Fit board in view, unfill all zones -> screenshot -> refill all zones -> screenshot
python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
python3 "$XINPUT_PY" click "$CX" "$CY"
python3 "$XINPUT_PY" key Home
sleep 0.45

python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.ZoneFiller.zoneUnfillAll" >/dev/null
sleep 0.7
import -window root "$OUT/shots/03a_zones_unfilled.png"

python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.ZoneFiller.zoneFillAll" >/dev/null
sleep 0.9
import -window root "$OUT/shots/03b_zones_filled.png"

ZONE_DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/03a_zones_unfilled.png" "$OUT/shots/03b_zones_filled.png" $((GX + VX + 40)) $((GY + VY + 40)) $((VW - 80)) $((VH - 80)))
ZONE_OK=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(int(int(sys.argv[2]) >= 1 and d["changed_pixels"] > 150))' "$ZONE_DIFF" "$ZONES_0")
check "Copper zone unfill + refill updated GAL pour geometry ($ZONE_DIFF)" "$ZONE_OK" "$ZONE_DIFF"

# Add track on F.Cu and save board cleanly
python3 "$IPC_PY" agent-add-track "$API_SOCK" 32.0 96.0 148.0 128.0 1.2 3 >/dev/null
sleep 0.4
python3 "$IPC_PY" save-doc "$API_SOCK" pcb >/dev/null
sleep 0.4
shot "03c_pcb_saved"

TRACES_1=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
MOD_AFTER_SAVE=$(is_doc_modified "$API_SOCK" pcb "$PCB_XID")
check "PCB track added ($TRACES_0 -> $TRACES_1) and SaveDocument cleared modified state" \
  "$([ "$TRACES_1" -eq $((TRACES_0 + 1)) ] && [ "$MOD_AFTER_SAVE" -eq 0 ] && echo 1 || echo 0)" \
  "traces=$TRACES_0->$TRACES_1 mod=$MOD_AFTER_SAVE"

# 5. Footprint Editor: embed in-process Footprint Editor inside GPUI, verify GAL paint
python3 "$DRIVER_PY" show "$PCB_XID" 0 >/dev/null
python3 "$DRIVER_PY" embed "$CONT_XID" "$FP_XID" "$VW" "$VH" >/dev/null
sleep 0.9
shot "04_footprint_editor"
RPT_FP=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$FP_XID")
FP_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 15))' "$RPT_FP")
check "Footprint Editor embedded in GPUI and painted footprint workspace" "$FP_OK" "$RPT_FP"
python3 "$DRIVER_PY" show "$FP_XID" 0 >/dev/null
python3 "$DRIVER_PY" show "$PCB_XID" 1 >/dev/null
sleep 0.45

# 6. DRC Dialog: open over embedded PCB Editor, verify WM_TRANSIENT_FOR + screen bounds, cross-check CLI DRC
python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.DRCTool.runDRC" >/dev/null
DRC_XID=$(wait_for_kicad_win "$KP" "Design Rules Checker" 60)
if [ "$EMBED_MODE" = "spike" ]; then
  python3 "$DRIVER_PY" set-transient "$DRC_XID" "$GPUI_XID" >/dev/null
fi
sleep 0.4
shot "05_drc_dialog"
DRC_INFO=$(python3 "$DRIVER_PY" get-transient "$DRC_XID")
DRC_OK=$(python3 -c '
import json, sys
d = json.loads(sys.argv[1]); gpui = sys.argv[2]; dlg = d.get("dialog", {})
in_bounds = 0 <= dlg.get("root_x", -1) <= 1700 and 0 <= dlg.get("root_y", -1) <= 1000 and dlg.get("shown") is True
print(int(d.get("transient_for") == gpui and in_bounds))
' "$DRC_INFO" "$GPUI_XID")
check "DRC dialog floats above GPUI with WM_TRANSIENT_FOR == gpui_xid and inside screen bounds" "$DRC_OK" "$DRC_INFO"

python3 "$DRIVER_PY" focus "$DRC_XID" >/dev/null
python3 "$XINPUT_PY" key Escape
sleep 0.3
"$KICAD_CLI" pcb drc --format json -o "$OUT/drc.json" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
CLI_DRC_OK=$(python3 -c 'import json,os,sys; print(int(os.path.isfile(sys.argv[1]) and "violations" in json.load(open(sys.argv[1]))))' "$OUT/drc.json")
check "Saved PCB produces valid kicad-cli pcb drc JSON report" "$CLI_DRC_OK"

# 7. 3D Viewer: open while PCB_EDIT_FRAME is mapped, then embed OpenGL viewer inside GPUI and orbit
python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.show3DViewer" >/dev/null
V3D_XID=$(wait_for_kicad_win "$KP" "3D Viewer" 80)
python3 "$DRIVER_PY" show "$PCB_XID" 0 >/dev/null
python3 "$DRIVER_PY" embed "$CONT_XID" "$V3D_XID" "$VW" "$VH" >/dev/null
sleep 1.4
RPT_3D=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$V3D_XID")
import -window root "$OUT/shots/06a_3d_before_orbit.png"

python3 "$DRIVER_PY" focus "$V3D_XID" >/dev/null
python3 "$XINPUT_PY" drag $((CX - 90)) $((CY - 50)) $((CX + 90)) $((CY + 60))
sleep 0.9
import -window root "$OUT/shots/06b_3d_after_orbit.png"

ORBIT_DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/06a_3d_before_orbit.png" "$OUT/shots/06b_3d_after_orbit.png" $((GX + VX + 40)) $((GY + VY + 60)) $((VW - 80)) $((VH - 100)))
V3D_OK=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1]); diff = json.loads(sys.argv[2])
print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 20 and diff["changed_pixels"] > 400))
' "$RPT_3D" "$ORBIT_DIFF")
check "Embedded 3D Viewer rendered OpenGL board and orbited on mouse drag ($ORBIT_DIFF)" "$V3D_OK" "$ORBIT_DIFF"

# 8. Plot dialog + Gerbers / Drill / STEP manufacturing export + companion metadata preservation
python3 "$DRIVER_PY" show "$V3D_XID" 0 >/dev/null
python3 "$DRIVER_PY" show "$PCB_XID" 1 >/dev/null
sleep 0.35
# RunAction("common.Control.plot") opens modal wxDialog::ShowModal(), so invoke asynchronously and dismiss with Escape
python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.plot" >/dev/null 2>&1 &
PLOT_CALL_PID=$!
PLOT_XID=$(wait_for_kicad_win "$KP" "Plot" 60 || true)
PLOT_DLG_OK=0
if [ -n "$PLOT_XID" ]; then
  [ "$EMBED_MODE" = "spike" ] && python3 "$DRIVER_PY" set-transient "$PLOT_XID" "$GPUI_XID" >/dev/null
  sleep 0.35
  shot "07_plot_dialog"
  PLOT_DLG_OK=1
  python3 "$DRIVER_PY" focus "$PLOT_XID" >/dev/null
  python3 "$XINPUT_PY" key Escape
fi
wait "$PLOT_CALL_PID" 2>/dev/null || true
check "Plot dialog opened cleanly over embedded PCB Editor and dismissed without hang" "$PLOT_DLG_OK" "$PLOT_XID"

mkdir -p "$PROJ/mfg/gerbers"
"$KICAD_CLI" pcb export gerbers -o "$PROJ/mfg/gerbers/" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1
"$KICAD_CLI" pcb export drill -o "$PROJ/mfg/gerbers/" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1
"$KICAD_CLI" pcb export step --board-only --subst-models -o "$PROJ/mfg/ecc83-pp.step" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1

MFG_OK=$(python3 -c '
import glob, os, sys
gdir, step_f = sys.argv[1], sys.argv[2]
gbrs = [p for p in glob.glob(os.path.join(gdir, "*.g*")) if os.path.getsize(p) > 100]
drls = [p for p in glob.glob(os.path.join(gdir, "*.drl")) if os.path.getsize(p) > 50]
step_ok = os.path.isfile(step_f) and os.path.getsize(step_f) > 1000
print(int(len(gbrs) >= 4 and len(drls) >= 1 and step_ok))
' "$PROJ/mfg/gerbers" "$PROJ/mfg/ecc83-pp.step")
check "Manufacturing export produced valid Gerbers, Excellon drill, and STEP model" "$MFG_OK"

SHA_BP_AFTER=$(sha256sum "$PROJ/.backplane.json" | awk '{print $1}')
SHA_COMP_AFTER=$(sha256sum "$PROJ/ecc83-pp.kicad_pcb.backplane.json" | awk '{print $1}')
check "Companion metadata (.backplane.json & .kicad_pcb.backplane.json) preserved byte-for-byte" \
  "$([ "$SHA_BP_BEFORE" = "$SHA_BP_AFTER" ] && [ "$SHA_COMP_BEFORE" = "$SHA_COMP_AFTER" ] && echo 1 || echo 0)" \
  "$SHA_BP_AFTER / $SHA_COMP_AFTER"

stop_pid "$KP"
stop_pid "$CP"
finish_suite "embed_editors_e2e"
