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
#
# Modes:
#   EMBED_MODE=stock|fork — drives Backplane's own ui:Edit, rail switches, and editor.kicad.* RPCs (zero driver placement)
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

# Record companion metadata hashes and baseline schematic/PCB counts before any GUI edits
SHA_BP_BEFORE=$(sha256sum "$PROJ/.backplane.json" | awk '{print $1}')
SHA_COMP_BEFORE=$(sha256sum "$PROJ/ecc83-pp.kicad_pcb.backplane.json" | awk '{print $1}')
SCH_WIRES_BEFORE=$(grep -c "(wire" "$PROJ/ecc83-pp.kicad_sch" || true)

if [ "$EMBED_MODE" = "spike" ]; then
  API_SOCK="$OUT/tmp/kicad/api.sock"

  # 1. Spawn ONE KiCad project manager suite process and pre-open PCB, Schematic, Symbol & Footprint editors
  "$KICAD_BIN" "$PROJ/ecc83-pp.kicad_pro" >"$OUT/kicad.log" 2>&1 &
  KP=$!
  track_pid "$KP"

  PM_XID=$(wait_for_kicad_win "$KP" "KiCad" 80)
  IPC_UP=0
  wait_for_ipc "$API_SOCK" 80 && IPC_UP=1
  VER_JSON=$(python3 "$IPC_PY" version "$API_SOCK" 2>/dev/null || echo "{}")
  check "Project Manager started single KiCad suite + live GUI IPC socket ($VER_JSON)" "$IPC_UP" "$VER_JSON"

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

  sleep 0.7
  for wxid in "$PM_XID" "$PCB_XID" "$SCH_XID" "$SYM_XID" "$FP_XID"; do
    python3 "$DRIVER_PY" withdraw "$wxid" >/dev/null
  done
  sleep 0.4

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

  # 2. Schematic Editor
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

  # 3. Symbol Editor
  python3 "$DRIVER_PY" show "$SCH_XID" 0 >/dev/null
  python3 "$DRIVER_PY" embed "$CONT_XID" "$SYM_XID" "$VW" "$VH" >/dev/null
  sleep 0.9
  shot "02_symbol_editor"
  RPT_SYM=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SYM_XID")
  SYM_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 15))' "$RPT_SYM")
  check "Symbol Editor embedded in GPUI and painted symbol workspace" "$SYM_OK" "$RPT_SYM"
  python3 "$DRIVER_PY" show "$SYM_XID" 0 >/dev/null

  # 4. PCB Editor
  python3 "$DRIVER_PY" embed "$CONT_XID" "$PCB_XID" "$VW" "$VH" >/dev/null
  sleep 0.9
  RPT_PCB=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$PCB_XID")
  PCB_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and (r.get("native",{}).get("painted") or 0) > 20))' "$RPT_PCB")
  check "PCB Editor (pcbnew) embedded in GPUI and painted board GAL canvas" "$PCB_OK" "$RPT_PCB"

  TRACES_0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
  ZONES_0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 16 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')

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

  # 5. Footprint Editor
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

  # 6. DRC Dialog
  python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.DRCTool.runDRC" >/dev/null
  DRC_XID=$(wait_for_kicad_win "$KP" "Design Rules Checker" 60)
  python3 "$DRIVER_PY" set-transient "$DRC_XID" "$GPUI_XID" >/dev/null
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

  # 7. 3D Viewer
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

  # 8. Plot dialog + manufacturing export
  python3 "$DRIVER_PY" show "$V3D_XID" 0 >/dev/null
  python3 "$DRIVER_PY" show "$PCB_XID" 1 >/dev/null
  sleep 0.35
  python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.plot" >/dev/null 2>&1 &
  PLOT_CALL_PID=$!
  PLOT_XID=$(wait_for_kicad_win "$KP" "Plot" 60 || true)
  PLOT_DLG_OK=0
  if [ -n "$PLOT_XID" ]; then
    python3 "$DRIVER_PY" set-transient "$PLOT_XID" "$GPUI_XID" >/dev/null
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
  finish_suite "embed_editors_e2e (spike)"
  exit $?
fi

# ------------------------------------------------------------------------------
# Product mode (stock | fork): ZERO driver placement!
# Backplane's own ui:Edit, rail clicks, and editor.kicad.* RPCs own KiCad.
# ------------------------------------------------------------------------------
cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":45000,"kind":"PCB","tag":"pcb_init"}},{"mark":"host_ready"}]
EOF

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 220 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

wait_for_log "$OUT/client.log" '"name":"host_ready","event":"mark"' 240
R_INIT=$(last_kicad_report "$OUT/client.log" "pcb_init")
read -r KP API_SOCK GPUI_XID CONT_XID PCB_XID < <(python3 - "$R_INIT" <<'PY'
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

IPC_UP=0
wait_for_ipc "$API_SOCK" 80 && IPC_UP=1
VER_JSON=$(python3 "$IPC_PY" version "$API_SOCK" 2>/dev/null || echo "{}")
check "Backplane ui:Edit started KiCad ($EMBED_MODE) + live GUI IPC socket ($VER_JSON)" \
  "$([ "$KP" -gt 0 ] && [ "$IPC_UP" -eq 1 ] && echo 1 || echo 0)" "$VER_JSON"

for _ in $(seq 50); do
  python3 "$IPC_PY" open-docs "$API_SOCK" pcb 2>/dev/null | grep -q '"count": 1' && break
  sleep 0.25
done

# 2. Switch to Schematic via Backplane's rail, draw wire interactively, Ctrl+S save, ERC + SVG export
click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:Schematic"
SCH_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "Schematic Editor" 160)
sleep 0.6
RPT_SCH=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SCH_XID")
SCH_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 20))' "$RPT_SCH")
check "Schematic Editor (eeschema) placed by Backplane client and rendered GAL canvas" "$SCH_OK" "$RPT_SCH"

read -r SX SY SW SH < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT_SCH")
# Aim inside the right-hand schematic canvas (past KiCad 10's left Properties dock)
SCX=$((SX + SW * 2 / 3))
SCY=$((SY + SH / 2))
python3 "$XINPUT_PY" click "$SCX" "$SCY"
sleep 0.35
python3 "$XINPUT_PY" key Home
sleep 0.4
python3 "$XINPUT_PY" key w
sleep 0.25
python3 "$XINPUT_PY" click $((SCX - 45)) "$SCY"
sleep 0.25
python3 "$XINPUT_PY" click $((SCX + 95)) "$SCY"
sleep 0.25
python3 "$XINPUT_PY" key End
sleep 0.2
python3 "$XINPUT_PY" key Escape
sleep 0.3
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

# 3. Symbol Editor: open via Backplane hub RPC editor.kicad.show -> client adopts/places inside pane
python3 "$XINPUT_PY" chord ctrl grave
sleep 0.15
hub_rpc "[{\"method\":\"editor.kicad.show\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"frame\":\"SYMBOL_EDITOR\"}}]" >/dev/null
SYM_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "Symbol Editor" 160)
sleep 0.6
shot "02_symbol_editor"
RPT_SYM=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SYM_XID")
SYM_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 15))' "$RPT_SYM")
check "Symbol Editor opened and placed by Backplane client inside viewer pane" "$SYM_OK" "$RPT_SYM"

# 4. PCB Editor: switch back via Backplane's Board rail, unfill/refill copper zones, add routed F.Cu track, save
click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:Board"
PCB_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "PCB Editor" 120)
sleep 0.5
RPT_PCB=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$PCB_XID")
PCB_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 20))' "$RPT_PCB")
check "PCB Editor (pcbnew) placed by Backplane client and painted board GAL canvas" "$PCB_OK" "$RPT_PCB"

TRACES_0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 11 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')
ZONES_0=$(python3 "$IPC_PY" items-count "$API_SOCK" pcb 16 | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')

read -r BX BY BW BH < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT_PCB")
python3 "$XINPUT_PY" click $((BX + BW * 2 / 3)) $((BY + BH / 2))
sleep 0.3
python3 "$XINPUT_PY" key Home
sleep 0.45

python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.ZoneFiller.zoneUnfillAll" >/dev/null
sleep 0.7
import -window root "$OUT/shots/03a_zones_unfilled.png"

python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.ZoneFiller.zoneFillAll" >/dev/null
sleep 0.9
import -window root "$OUT/shots/03b_zones_filled.png"

ZONE_DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/03a_zones_unfilled.png" "$OUT/shots/03b_zones_filled.png" $((BX + BW / 3)) $((BY + 50)) $((BW * 2 / 3 - 50)) $((BH - 100)))
ZONE_OK=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(int(int(sys.argv[2]) >= 1 and d["changed_pixels"] > 150))' "$ZONE_DIFF" "$ZONES_0")
check "Copper zone unfill + refill updated GAL pour geometry ($ZONE_DIFF)" "$ZONE_OK" "$ZONE_DIFF"

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

# 5. Footprint Editor: open via Backplane hub RPC editor.kicad.show -> client places inside pane
python3 "$XINPUT_PY" chord ctrl grave
sleep 0.15
hub_rpc "[{\"method\":\"editor.kicad.show\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"frame\":\"FOOTPRINT_EDITOR\"}}]" >/dev/null
FP_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "Footprint Editor" 160)
sleep 0.6
shot "04_footprint_editor"
RPT_FP=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$FP_XID")
FP_OK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 15))' "$RPT_FP")
check "Footprint Editor opened and placed by Backplane client inside viewer pane" "$FP_OK" "$RPT_FP"

click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:Board"
PCB_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "PCB Editor" 120)
sleep 0.4

# 6. DRC Dialog: Backplane's own client/fork sets WM_TRANSIENT_FOR to GPUI top-level (zero driver set-transient!)
python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.DRCTool.runDRC" >/dev/null
DRC_XID=$(wait_for_kicad_win "$KP" "Design Rules Checker" 60)
sleep 0.45
shot "05_drc_dialog"
DRC_INFO=$(python3 "$DRIVER_PY" get-transient "$DRC_XID")
DRC_OK=$(python3 -c '
import json, sys
d = json.loads(sys.argv[1]); gpui = sys.argv[2]; dlg = d.get("dialog", {})
in_bounds = 0 <= dlg.get("root_x", -1) <= 1700 and 0 <= dlg.get("root_y", -1) <= 1000 and dlg.get("shown") is True
print(int(d.get("transient_for") == gpui and in_bounds))
' "$DRC_INFO" "$GPUI_XID")
check "DRC dialog floats above GPUI with Backplane-managed WM_TRANSIENT_FOR == gpui_xid and inside screen bounds" "$DRC_OK" "$DRC_INFO"

read -r DFX DFY DCX DCY < <(python3 -c 'import json,sys; g=json.loads(sys.argv[1]).get("dialog",{}); print(g.get("root_x",600)+120, g.get("root_y",300)+18, g.get("root_x",600)+g.get("w",400)-108, g.get("root_y",300)+g.get("h",300)-28)' "$DRC_INFO")
python3 "$XINPUT_PY" click "$DFX" "$DFY"
sleep 0.12
python3 "$XINPUT_PY" key Escape
sleep 0.2
python3 "$XINPUT_PY" click "$DCX" "$DCY"
sleep 0.25

"$KICAD_CLI" pcb drc --format json -o "$OUT/drc.json" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
CLI_DRC_OK=$(python3 -c 'import json,os,sys; print(int(os.path.isfile(sys.argv[1]) and "violations" in json.load(open(sys.argv[1]))))' "$OUT/drc.json")
check "Saved PCB produces valid kicad-cli pcb drc JSON report" "$CLI_DRC_OK"

# 7. 3D Viewer: switch via Backplane's 3D rail, let OpenGL settle, and orbit by mouse drag
click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:3D"
V3D_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "3D Viewer" 160)
sleep 1.4
RPT_3D=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$V3D_XID")
import -window root "$OUT/shots/06a_3d_before_orbit.png"

read -r X3D Y3D W3D H3D < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT_3D")
python3 "$XINPUT_PY" click $((X3D + W3D / 2)) $((Y3D + H3D / 2))
sleep 0.25
python3 "$XINPUT_PY" drag $((X3D + W3D / 2 - 90)) $((Y3D + H3D / 2 - 50)) $((X3D + W3D / 2 + 90)) $((Y3D + H3D / 2 + 60))
sleep 0.9
import -window root "$OUT/shots/06b_3d_after_orbit.png"

ORBIT_DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/06a_3d_before_orbit.png" "$OUT/shots/06b_3d_after_orbit.png" $((X3D + 40)) $((Y3D + 60)) $((W3D - 80)) $((H3D - 100)))
V3D_OK=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1]); diff = json.loads(sys.argv[2])
print(int(bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 20 and diff["changed_pixels"] > 400))
' "$RPT_3D" "$ORBIT_DIFF")
check "Embedded 3D Viewer placed by Backplane client rendered OpenGL board and orbited on mouse drag ($ORBIT_DIFF)" "$V3D_OK" "$ORBIT_DIFF"

# 8. Plot dialog + Gerbers / Drill / STEP manufacturing export + companion metadata preservation
click_gpui_control "$OUT/client.log" "$GPUI_XID" "rail:Board"
PCB_XID=$(wait_for_embedded_frame "$GPUI_XID" "$CONT_XID" "$KP" "PCB Editor" 120)
sleep 0.35

python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.plot" >/dev/null 2>&1 &
PLOT_CALL_PID=$!
PLOT_XID=$(wait_for_kicad_win "$KP" "Plot" 60 || true)
PLOT_DLG_OK=0
if [ -n "$PLOT_XID" ]; then
  sleep 0.45
  shot "07_plot_dialog"
  PLOT_INFO=$(python3 "$DRIVER_PY" get-transient "$PLOT_XID")
  PLOT_TRANS_OK=$(python3 -c '
import json, sys
d = json.loads(sys.argv[1]); gpui = sys.argv[2]; mode = sys.argv[3]; dlg = d.get("dialog", {})
ok = dlg.get("shown") is True and (mode == "spike" or d.get("transient_for") == gpui)
print(int(ok))
' "$PLOT_INFO" "$GPUI_XID" "$EMBED_MODE")
  read -r PX PY < <(python3 -c 'import json,sys; g=json.loads(sys.argv[1]).get("dialog",{}); print(g.get("root_x",600)+g.get("w",400)//2, g.get("root_y",300)+20)' "$PLOT_INFO")
  python3 "$XINPUT_PY" click "$PX" "$PY"
  sleep 0.15
  python3 "$XINPUT_PY" key Escape
  wait "$PLOT_CALL_PID" 2>/dev/null || true
  sleep 0.2
  if [ "$PLOT_TRANS_OK" -eq 1 ] && python3 "$IPC_PY" ping "$API_SOCK" >/dev/null 2>&1; then
    PLOT_DLG_OK=1
  fi
else
  wait "$PLOT_CALL_PID" 2>/dev/null || true
fi
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
COMP_OK=0
SHA_COMP_AFTER="migrated-to-kicad_pcb"
if [ -f "$PROJ/ecc83-pp.kicad_pcb.backplane.json" ]; then
  SHA_COMP_AFTER=$(sha256sum "$PROJ/ecc83-pp.kicad_pcb.backplane.json" | awk '{print $1}')
  [ "$SHA_COMP_BEFORE" = "$SHA_COMP_AFTER" ] && COMP_OK=1
elif [ "$EMBED_MODE" = "fork" ] && grep -Fq '(custom_property "backplane_note" "preserve-across-gui-save-and-plot")' "$PROJ/ecc83-pp.kicad_pcb"; then
  # KiCad 10.99-bp loads item custom_properties from .kicad_pcb.backplane.json into native (custom_property ...) and removes empty companions.
  COMP_OK=1
fi
check "Companion metadata (.backplane.json & custom_properties) preserved across GUI save and plot" \
  "$([ "$SHA_BP_BEFORE" = "$SHA_BP_AFTER" ] && [ "$COMP_OK" -eq 1 ] && echo 1 || echo 0)" \
  "$SHA_BP_AFTER / $SHA_COMP_AFTER"

hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" >/dev/null || true
stop_pid "$CP"
stop_pid "$KP"
finish_suite "embed_editors_e2e ($EMBED_MODE)"
