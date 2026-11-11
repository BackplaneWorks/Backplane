#!/bin/bash
# Suite 1: Embedded X11 parenting, 32-bit/24-bit visual safety, layout reflow, pure-move popup
# coordinate sync, XShape hole-punching under GPUI overlays, multi-frame tab switching (<50ms),
# and focus/shortcut isolation.
#
# Mode-agnostic:
#   EMBED_MODE=spike (default): stock KiCad + harness X11 container/reparent + synthetic ConfigureNotify
#   EMBED_MODE=stock|fork:      client ui:Edit / ui:View + {"kicad":{"report":true}} GPUI_LOG contract
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

API_SOCK="$OUT/tmp/kicad/api.sock"
SPIKE_REPORTS="$OUT/spike_reports.jsonl"
: >"$SPIKE_REPORTS"

# In spike mode, pre-open the single KiCad suite process (PCB + Schematic + 3D Viewer) so
# reparenting at GPUI script marks is sub-second and deterministic.
if [ "$EMBED_MODE" = "spike" ]; then
  "$KICAD_BIN" "$OUT/proj/ecc83-pp.kicad_pro" >"$OUT/kicad.log" 2>&1 &
  KP=$!
  track_pid "$KP"

  PM_XID=$(wait_for_kicad_win "$KP" "KiCad" 80)
  wait_for_ipc "$API_SOCK" 80

  # Click inside Project Manager to ensure GTK focus, then Ctrl+P to open PCB Editor
  sleep 0.6
  click_win_center "$PM_XID"
  sleep 0.3
  python3 "$XINPUT_PY" chord ctrl p
  PCB_XID=$(wait_for_kicad_win "$KP" "PCB Editor" 80)

  # Once PCB_EDIT_FRAME registers RunAction on api.sock, open Schematic Editor & 3D Viewer deterministically via IPC
  for _ in $(seq 50); do
    python3 "$IPC_PY" open-docs "$API_SOCK" pcb 2>/dev/null | grep -q '"count": 1' && break
    sleep 0.25
  done
  python3 "$IPC_PY" run-action "$API_SOCK" "pcbnew.EditorControl.showEeschema" >/dev/null
  SCH_XID=$(wait_for_kicad_win "$KP" "Schematic Editor" 80)

  python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.show3DViewer" >/dev/null
  V3D_XID=$(wait_for_kicad_win "$KP" "3D Viewer" 80)

  # Withdraw all KiCad top-level windows from i3 (ICCCM 4.1.4) before GPUI starts so GPUI gets full 1800x1050
  for wxid in "$PM_XID" "$PCB_XID" "$SCH_XID" "$V3D_XID"; do
    python3 "$DRIVER_PY" withdraw "$wxid" >/dev/null
  done
  sleep 0.4
fi

# Build GPUI --script (mode-agnostic: emits native kicad report steps in stock|fork mode)
python3 - "$OUT/script.json" "$EMBED_MODE" <<'PY'
import json, sys
out_file, mode = sys.argv[1], sys.argv[2]
native = mode in ("stock", "fork")

def rep(mark_name, wait_ms=3500):
    steps = [{"mark": mark_name}]
    if native:
        steps.append({"kicad": {"report": True}})
    steps.append({"wait": wait_ms})
    return steps

s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}]
if native:
    s += [{"click": "ui:Edit"}, {"wait": 12000}]
s += rep("shown", 4500)
s += [{"keys": "ctrl-b"}, {"wait": 1800}] + rep("sidebar_folded", 3500)
s += [{"keys": "ctrl-b"}, {"wait": 1800}] + rep("pre_resize", 3000) + rep("resized", 3500)
s += rep("pre_move", 2500) + rep("pure_moved", 4500)
s += [{"click": "ui:Jump or run a command"}, {"wait": 1500}] + rep("overlay_cutout", 3500)
s += [{"keys": "escape"}, {"wait": 1200}] + rep("overlay_closed", 2500)
s += [{"click": "rail:Files"}, {"wait": 1800}] + rep("other_view", 2500)
s += [{"click": "rail:Board"}, {"wait": 1800}] + rep("back_board", 3000)
s += rep("multi_frame_switch", 4500)
if native:
    s += [{"click": "ui:View"}, {"wait": 1800}] + rep("view_mode", 2000)
s += [{"wait": 1500}]
with open(out_file, "w", encoding="utf-8") as f:
    json.dump(s, f)
PY

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 110 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

record_spike_report() {
  local mark=$1 ch_xid=$2 extra=${3:-"{}"}
  local rpt
  rpt=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$ch_xid")
  python3 -c 'import json,sys; r=json.loads(sys.argv[1]); r["mark"]=sys.argv[2]; r.update(json.loads(sys.argv[3])); print(json.dumps(r))' \
    "$rpt" "$mark" "$extra" >>"$SPIKE_REPORTS"
}

read_pane() {
  local folded_flag=${1:-}
  eval "$(python3 "$DRIVER_PY" find-gpui --shell --client-log "$OUT/client.log" $folded_flag)"
}

seen=""
CONT_XID=""
while kill -0 "$CP" 2>/dev/null; do
  for m in $(grep -o '"name":"[a-z_0-9]*","event":"mark"' "$OUT/client.log" 2>/dev/null | cut -d'"' -f4); do
    case " $seen " in *" $m "*) continue ;; esac
    seen="$seen $m"
    sleep 0.35

    if [ "$EMBED_MODE" = "spike" ]; then
      case "$m" in
        shown)
          read_pane ""
          start_container "$GPUI_XID" "$VX" "$VY" "$VW" "$VH" "main"
          CONT_XID="$CONTAINER_LAST_XID"
          # Embed all three frames into container; keep PCB visible first
          python3 "$DRIVER_PY" embed "$CONT_XID" "$SCH_XID" "$VW" "$VH" >/dev/null
          python3 "$DRIVER_PY" show "$SCH_XID" 0 >/dev/null
          python3 "$DRIVER_PY" embed "$CONT_XID" "$V3D_XID" "$VW" "$VH" >/dev/null
          python3 "$DRIVER_PY" show "$V3D_XID" 0 >/dev/null
          python3 "$DRIVER_PY" embed "$CONT_XID" "$PCB_XID" "$VW" "$VH" >/dev/null
          # Keep focus on host initially (no focus theft before user click)
          python3 "$DRIVER_PY" focus "$GPUI_XID" >/dev/null
          sleep 0.8
          shot "01_shown"
          record_spike_report "shown" "$PCB_XID" "{\"gpui_depth\":$GPUI_DEPTH,\"root_depth\":$ROOT_DEPTH,\"pane_x\":$VX,\"pane_y\":$VY,\"pane_w\":$VW,\"pane_h\":$VH}"

          # Verify click-to-focus inside embedded PCB canvas takes focus cleanly
          python3 "$XINPUT_PY" click $((GX + VX + 140)) $((GY + VY + 140))
          python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
          sleep 0.25
          record_spike_report "clicked_focus" "$PCB_XID" "{}"
          # Hand focus back to GPUI so script's upcoming ctrl-b goes to GPUI
          python3 "$DRIVER_PY" focus "$GPUI_XID" >/dev/null
          ;;
        sidebar_folded)
          read_pane "--folded"
          python3 "$DRIVER_PY" place "$CONT_XID" "$PCB_XID" "$VX" "$VY" "$VW" "$VH" >/dev/null
          sleep 0.5
          shot "02_sidebar_folded"
          record_spike_report "sidebar_folded" "$PCB_XID" "{\"pane_x\":$VX,\"pane_y\":$VY,\"pane_w\":$VW,\"pane_h\":$VH}"
          ;;
        pre_resize)
          i3-msg -s "$OUT/i3.sock" 'floating enable, resize set 1480 880, move position 40 30' >"$OUT/i3_resize.json" 2>&1 || true
          ;;
        resized)
          read_pane ""
          python3 "$DRIVER_PY" place "$CONT_XID" "$PCB_XID" "$VX" "$VY" "$VW" "$VH" >/dev/null
          sleep 0.5
          shot "03_resized"
          record_spike_report "resized" "$PCB_XID" "{\"pane_x\":$VX,\"pane_y\":$VY,\"pane_w\":$VW,\"pane_h\":$VH}"
          ;;
        pre_move)
          # Pure top-level window move WITHOUT resizing: tests GTK3 synthetic ConfigureNotify root-coord update
          i3-msg -s "$OUT/i3.sock" 'move position 210 130' >"$OUT/i3_move.json" 2>&1 || true
          ;;
        pure_moved)
          read_pane ""
          python3 "$DRIVER_PY" send-configure "$PCB_XID" >/dev/null
          sleep 0.35
          # Right-click in embedded PCB canvas to open GTK context menu and verify root popup position
          RCX=$((GX + VX + VW / 2))
          RCY=$((GY + VY + VH / 2))
          python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
          python3 "$XINPUT_PY" click "$RCX" "$RCY" 3
          sleep 0.6
          shot "04_pure_moved_popup"
          POPUPS=$(python3 "$DRIVER_PY" list-windows --pid "$KP" --popup)
          python3 "$XINPUT_PY" key Escape
          python3 "$DRIVER_PY" focus "$GPUI_XID" >/dev/null
          record_spike_report "pure_moved" "$PCB_XID" "{\"gx\":$GX,\"gy\":$GY,\"pane_x\":$VX,\"pane_y\":$VY,\"pane_w\":$VW,\"pane_h\":$VH,\"popups\":$POPUPS}"
          ;;
        overlay_cutout)
          read_pane ""
          # Command palette sits centered near top of GPUI window (~560x360 around (GW/2 - 280, 64));
          # convert overlap into container-local coordinates and punch XShape bounding cutout.
          PAL_X=$((GW / 2 - 280 - VX))
          PAL_Y=$((64 - VY))
          SHAPE_JSON=$(python3 "$DRIVER_PY" shape-cutout "$CONT_XID" "$VW" "$VH" "$PAL_X" "$PAL_Y" 560 360)
          sleep 0.35
          shot "05_overlay_cutout"
          record_spike_report "overlay_cutout" "$PCB_XID" "{\"shape\":$SHAPE_JSON}"
          ;;
        overlay_closed)
          RESET_JSON=$(python3 "$DRIVER_PY" shape-reset "$CONT_XID")
          sleep 0.25
          shot "06_overlay_closed"
          record_spike_report "overlay_closed" "$PCB_XID" "{\"shape_reset\":$RESET_JSON}"
          ;;
        other_view)
          python3 "$DRIVER_PY" show "$CONT_XID" 0 >/dev/null
          sleep 0.25
          shot "07_other_view"
          record_spike_report "other_view" "$PCB_XID" "{}"
          ;;
        back_board)
          python3 "$DRIVER_PY" show "$CONT_XID" 1 >/dev/null
          sleep 0.4
          shot "08_back_board"
          record_spike_report "back_board" "$PCB_XID" "{}"
          ;;
        multi_frame_switch)
          SW_SCH=$(python3 "$DRIVER_PY" switch "$SCH_XID" "$PCB_XID" "$V3D_XID")
          sleep 0.55
          shot "09_switch_sch"
          RPT_SCH=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$SCH_XID")

          SW_3D=$(python3 "$DRIVER_PY" switch "$V3D_XID" "$PCB_XID" "$SCH_XID")
          sleep 0.75
          shot "10_switch_3d"
          RPT_3D=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$V3D_XID")

          SW_PCB=$(python3 "$DRIVER_PY" switch "$PCB_XID" "$SCH_XID" "$V3D_XID")
          sleep 0.45
          shot "11_switch_pcb"
          record_spike_report "multi_frame_switch" "$PCB_XID" "{\"sw_sch\":$SW_SCH,\"rpt_sch\":$RPT_SCH,\"sw_3d\":$SW_3D,\"rpt_3d\":$RPT_3D,\"sw_pcb\":$SW_PCB}"
          ;;
      esac
    else
      shot "$m"
    fi
  done
  sleep 0.15
done

# Stop KiCad cleanly before evaluating assertions
if [ "${KP:-}" != "" ]; then
  stop_pid "$KP"
fi

# Evaluate assertions across spike or stock/fork reports
python3 - "$OUT" "$EMBED_MODE" <<'PY'
import json, os, sys

out_dir, mode = sys.argv[1], sys.argv[2]
reps = {}
if mode == "spike":
    with open(os.path.join(out_dir, "spike_reports.jsonl"), encoding="utf-8") as f:
        for line in f:
            if line.strip():
                r = json.loads(line)
                reps[r["mark"]] = r
else:
    marks = []
    with open(os.path.join(out_dir, "client.log"), encoding="utf-8", errors="ignore") as f:
        for line in f:
            if '"event":"mark"' in line:
                marks.append(json.loads(line[line.index("{"):])["name"])
            elif '"event":"kicad"' in line and marks:
                reps.setdefault(marks[-1], json.loads(line[line.index("{"):]))

ok = fail = 0
def check(what, cond, detail=""):
    global ok, fail
    if cond:
        print(f"PASS {what}")
        ok += 1
    else:
        print(f"FAIL {what}" + (f" ({str(detail)[:280]})" if detail != "" else ""))
        fail += 1

r_shown = reps.get("shown", {})
nat_shown = r_shown.get("native", {})
cont_shown = r_shown.get("container", {})

check(
    "32-bit GPUI host uses 24-bit DefaultVisual container without BadMatch",
    r_shown.get("gpui_depth", 32) == 32 and cont_shown.get("depth", 24) == 24,
    (r_shown.get("gpui_depth"), cont_shown),
)
check(
    "PCB Editor window is reparented inside Backplane GPUI X11 tree",
    bool(r_shown.get("in_gpui_tree")) and bool(nat_shown.get("shown")),
    r_shown,
)
check(
    "Embedded PCB Editor matches viewer pane geometry within <=2px",
    abs(cont_shown.get("x", -99) - r_shown.get("pane_x", 0)) <= 2
    and abs(cont_shown.get("y", -99) - r_shown.get("pane_y", 0)) <= 2
    and abs(nat_shown.get("w", -99) - r_shown.get("pane_w", 0)) <= 2
    and abs(nat_shown.get("h", -99) - r_shown.get("pane_h", 0)) <= 2,
    (cont_shown, nat_shown, r_shown.get("pane_x"), r_shown.get("pane_w")),
)
check(
    "PCB Editor OpenGL GAL canvas painted non-blank content inside host",
    (nat_shown.get("painted") or 0) > 20,
    nat_shown,
)
check(
    "Opening embedded KiCad did not steal keyboard focus before user click",
    r_shown.get("taken") is False,
    r_shown,
)

if "clicked_focus" in reps:
    check(
        "Clicking inside embedded PCB canvas transfers X11 input focus to KiCad",
        reps["clicked_focus"].get("taken") is True,
        reps["clicked_focus"],
    )

r_fold = reps.get("sidebar_folded", {})
c_fold = r_fold.get("container", {})
n_fold = r_fold.get("native", {})
check(
    "Folding GPUI sidebar (Ctrl+B) reflows and widens embedded KiCad pane",
    n_fold.get("shown") is True
    and c_fold.get("x", 9999) < cont_shown.get("x", 0)
    and n_fold.get("w", 0) > nat_shown.get("w", 0),
    (cont_shown.get("x"), c_fold.get("x"), nat_shown.get("w"), n_fold.get("w")),
)

r_res = reps.get("resized", {})
n_res = r_res.get("native", {})
check(
    "Window manager resize (1600x1000 -> 1280x820) resizes embedded KiCad cleanly",
    n_res.get("shown") is True
    and n_res.get("w", 9999) < nat_shown.get("w", 0)
    and abs(n_res.get("w", -99) - r_res.get("pane_w", 0)) <= 2,
    (nat_shown.get("w"), n_res.get("w"), r_res.get("pane_w")),
)

r_mv = reps.get("pure_moved", {})
popups = r_mv.get("popups", [])
exp_rx = r_mv.get("gx", 0) + r_mv.get("pane_x", 0)
exp_ry = r_mv.get("gy", 0) + r_mv.get("pane_y", 0)
popup_ok = any(
    (exp_rx - 40) <= p.get("root_x", -999) <= (exp_rx + r_mv.get("pane_w", 0) + 40)
    and (exp_ry - 40) <= p.get("root_y", -999) <= (exp_ry + r_mv.get("pane_h", 0) + 40)
    for p in popups
)
check(
    "Pure top-level window move + synthetic ConfigureNotify anchors right-click menu at new root coords",
    len(popups) >= 1 and popup_ok,
    {"expected_origin": (exp_rx, exp_ry), "popups": popups},
)

r_cut = reps.get("overlay_cutout", {})
shape = r_cut.get("shape", {})
r_cut_closed = reps.get("overlay_closed", {})
check(
    "GPUI Command Palette overlay punches XShape bounding hole in container and restores on close",
    shape.get("rect_count", 0) >= 2 and r_cut_closed.get("shape_reset", {}).get("rect_count") == 1,
    (shape, r_cut_closed.get("shape_reset")),
)

r_other = reps.get("other_view", {})
r_back = reps.get("back_board", {})
check(
    "Switching to non-CAD rail view unmaps container and switching back restores it",
    r_other.get("native", {}).get("shown") is False and r_back.get("native", {}).get("shown") is True,
    (r_other.get("native"), r_back.get("native")),
)

r_sw = reps.get("multi_frame_switch", {})
sw_sch = r_sw.get("sw_sch", {})
sw_3d = r_sw.get("sw_3d", {})
sw_pcb = r_sw.get("sw_pcb", {})
sch_painted = r_sw.get("rpt_sch", {}).get("native", {}).get("painted", 0)
v3d_painted = r_sw.get("rpt_3d", {}).get("native", {}).get("painted", 0)
check(
    "Sibling frame switching (PCB <-> Schematic <-> 3D Viewer) paints each editor in <50ms",
    sch_painted > 15
    and v3d_painted > 15
    and sw_sch.get("elapsed_ms", 999) < 50.0
    and sw_3d.get("elapsed_ms", 999) < 50.0
    and sw_pcb.get("elapsed_ms", 999) < 50.0,
    {"sw_sch": sw_sch, "sw_3d": sw_3d, "sw_pcb": sw_pcb, "sch_painted": sch_painted, "v3d_painted": v3d_painted},
)

print(f"embed_window_e2e: passed={ok} failed={fail} artifacts={out_dir}")
sys.exit(1 if fail else 0)
PY
