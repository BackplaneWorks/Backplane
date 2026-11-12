#!/bin/bash
# Suite 1: Embedded X11 parenting, 32-bit/24-bit visual safety, layout reflow, pure-move popup
# coordinate sync, XShape hole-punching under GPUI overlays, multi-frame tab switching,
# and focus/shortcut isolation.
#
# Mode-agnostic:
#   EMBED_MODE=spike (default): stock KiCad + harness X11 container/reparent + synthetic ConfigureNotify
#   EMBED_MODE=stock|fork:      Backplane's own ui:Edit / ui:View + hub RPCs + {"kicad":...} GPUI_METRIC contract
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

# In spike mode ONLY, pre-open the single KiCad suite process so reparenting at GPUI script marks is sub-second.
# In stock/fork mode, the hub spawns KiCad when the client clicks ui:Edit and the client places/adopts it itself.
if [ "$EMBED_MODE" = "spike" ]; then
  "$KICAD_BIN" "$OUT/proj/ecc83-pp.kicad_pro" >"$OUT/kicad.log" 2>&1 &
  KP=$!
  track_pid "$KP"

  PM_XID=$(wait_for_kicad_win "$KP" "KiCad" 80)
  wait_for_ipc "$API_SOCK" 80

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

  python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.show3DViewer" >/dev/null
  V3D_XID=$(wait_for_kicad_win "$KP" "3D Viewer" 80)

  for wxid in "$PM_XID" "$PCB_XID" "$SCH_XID" "$V3D_XID"; do
    python3 "$DRIVER_PY" withdraw "$wxid" >/dev/null
  done
  sleep 0.4
fi

# Build GPUI --script
python3 - "$OUT/script.json" "$EMBED_MODE" <<'PY'
import json, sys
out_file, mode = sys.argv[1], sys.argv[2]

if mode == "spike":
    def rep(mark_name, wait_ms=3500):
        return [{"mark": mark_name}, {"wait": wait_ms}]
    s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}]
    s += rep("shown", 4500)
    s += [{"keys": "ctrl-b"}, {"wait": 1800}] + rep("sidebar_folded", 3500)
    s += [{"keys": "ctrl-b"}, {"wait": 1800}] + rep("pre_resize", 3000) + rep("resized", 3500)
    s += rep("pre_move", 2500) + rep("pure_moved", 4500)
    s += [{"click": "ui:Jump or run a command"}, {"wait": 1500}] + rep("overlay_cutout", 3500)
    s += [{"keys": "escape"}, {"wait": 1200}] + rep("overlay_closed", 2500)
    s += [{"click": "rail:Files"}, {"wait": 1800}] + rep("other_view", 2500)
    s += [{"click": "rail:Board"}, {"wait": 1800}] + rep("back_board", 3000)
    s += rep("multi_frame_switch", 4500)
    s += [{"wait": 1500}]
else:
    s = [
        {"select": 0}, {"wait": 2000}, {"click": "rail:Board"}, {"wait": 2000},
        {"click": "ui:Edit"},
        {"kicad": {"wait_painted": 40000, "kind": "PCB", "tag": "shown"}},
        {"mark": "01_shown"}, {"wait": 2800},
        {"kicad": {"report": True, "tag": "clicked_focus"}},
        {"mark": "handback_focus"}, {"wait": 1800},
        {"kicad": {"report": True, "tag": "focus_returned"}},
        {"keys": "ctrl-b"}, {"wait": 1200},
        {"kicad": {"report": True, "tag": "sidebar_folded"}},
        {"mark": "02_sidebar_folded"}, {"wait": 1000},
        {"keys": "ctrl-b"}, {"wait": 1000},
        {"mark": "pre_resize"}, {"wait": 2200},
        {"kicad": {"report": True, "tag": "resized"}},
        {"mark": "03_resized"}, {"wait": 1000},
        {"mark": "pre_move"}, {"wait": 2200},
        {"kicad": {"report": True, "tag": "moved_pre_popup"}},
        {"mark": "pure_moved"}, {"wait": 3800},
        {"keys": "f1"}, {"wait": 800},
        {"kicad": {"report": True, "tag": "overlay_cutout"}},
        {"mark": "05_overlay_cutout"}, {"wait": 900},
        {"keys": "f1"}, {"wait": 800},
        {"kicad": {"report": True, "tag": "overlay_closed"}},
        {"mark": "06_overlay_closed"}, {"wait": 900},
        {"click": "rail:Files"}, {"wait": 1000},
        {"kicad": {"report": True, "tag": "other_view"}},
        {"mark": "07_other_view"}, {"wait": 900},
        {"click": "rail:Board"}, {"wait": 1000},
        {"kicad": {"report": True, "tag": "back_board"}},
        {"mark": "08_back_board"}, {"wait": 900},
        # Open Schematic and 3D Viewer via Backplane's own rail so all three editors are warm
        {"click": "rail:Schematic"},
        {"kicad": {"wait_painted": 40000, "kind": "SCH", "tag": "cold_sch"}},
        {"mark": "09_switch_sch"}, {"wait": 900},
        {"click": "rail:3D"},
        {"kicad": {"wait_painted": 40000, "kind": "PCB_3D", "tag": "cold_3d"}},
        {"mark": "10_switch_3d"}, {"wait": 900},
        # Warm switches driven strictly by Backplane's rail
        {"click": "rail:Board"},
        {"kicad": {"wait_painted": 5000, "kind": "PCB", "tag": "warm_pcb"}},
        {"click": "rail:Schematic"},
        {"kicad": {"wait_painted": 5000, "kind": "SCH", "tag": "warm_sch"}},
        {"click": "rail:3D"},
        {"kicad": {"wait_painted": 5000, "kind": "PCB_3D", "tag": "warm_3d"}},
        {"click": "rail:Board"},
        {"kicad": {"wait_painted": 5000, "kind": "PCB", "tag": "warm_pcb_2"}},
        {"mark": "11_switch_pcb"}, {"wait": 900},
        # A real agent (agent:*) edits the board: KiCad opens its Agents pane beside the canvas.
        # The person keeps switching tabs; the PCB must still repaint at once.
        {"mark": "agent_commit"}, {"wait": 2500},
        {"click": "rail:Schematic"},
        {"kicad": {"wait_painted": 5000, "kind": "SCH", "tag": "agent_warm_sch"}},
        {"click": "rail:3D"},
        {"kicad": {"wait_painted": 5000, "kind": "PCB_3D", "tag": "agent_warm_3d"}},
        {"click": "rail:Board"},
        {"kicad": {"wait_painted": 5000, "kind": "PCB", "tag": "agent_warm_pcb"}},
        {"mark": "13_agent_switch_pcb"}, {"wait": 900},
        # Return to Backplane View mode (KiCad stays warm in hub)
        {"click": "ui:View"}, {"wait": 1200},
        {"kicad": {"report": True, "tag": "view_mode"}},
        {"mark": "12_view_mode"}, {"wait": 1200},
    ]

with open(out_file, "w", encoding="utf-8") as f:
    json.dump(s, f)
PY

HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 220 >"$OUT/client.log" 2>&1 &
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
    sleep 0.25

    if [ "$EMBED_MODE" = "spike" ]; then
      case "$m" in
        shown)
          read_pane ""
          start_container "$GPUI_XID" "$VX" "$VY" "$VW" "$VH" "main"
          CONT_XID="$CONTAINER_LAST_XID"
          python3 "$DRIVER_PY" embed "$CONT_XID" "$SCH_XID" "$VW" "$VH" >/dev/null
          python3 "$DRIVER_PY" show "$SCH_XID" 0 >/dev/null
          python3 "$DRIVER_PY" embed "$CONT_XID" "$V3D_XID" "$VW" "$VH" >/dev/null
          python3 "$DRIVER_PY" show "$V3D_XID" 0 >/dev/null
          python3 "$DRIVER_PY" embed "$CONT_XID" "$PCB_XID" "$VW" "$VH" >/dev/null
          python3 "$DRIVER_PY" focus "$GPUI_XID" >/dev/null
          sleep 0.8
          shot "01_shown"
          record_spike_report "shown" "$PCB_XID" "{\"gpui_depth\":$GPUI_DEPTH,\"root_depth\":$ROOT_DEPTH,\"pane_x\":$VX,\"pane_y\":$VY,\"pane_w\":$VW,\"pane_h\":$VH}"

          python3 "$XINPUT_PY" click $((GX + VX + 140)) $((GY + VY + 140))
          python3 "$DRIVER_PY" focus "$PCB_XID" >/dev/null
          sleep 0.25
          record_spike_report "clicked_focus" "$PCB_XID" "{}"
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
          i3-msg -s "$OUT/i3.sock" 'move position 210 130' >"$OUT/i3_move.json" 2>&1 || true
          ;;
        pure_moved)
          read_pane ""
          python3 "$DRIVER_PY" send-configure "$PCB_XID" >/dev/null
          sleep 0.35
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
      # Product mode (stock | fork): ZERO driver placement! Only screenshots, WM resize/move, and real XTest clicks/keys.
      case "$m" in
        01_shown)
          shot "01_shown"
          R_SHOWN=$(last_kicad_report "$OUT/client.log" "shown")
          read -r RX RY < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); sr=r.get("shown_root") or {}; print(int(sr.get("x",800)), int(sr.get("y",400)))' "$R_SHOWN")
          python3 "$XINPUT_PY" click $((RX + 140)) $((RY + 140))
          sleep 0.35
          ;;
        handback_focus)
          python3 "$XINPUT_PY" chord ctrl grave
          sleep 0.25
          ;;
        pre_resize)
          i3-msg -s "$OUT/i3.sock" 'floating enable, resize set 1480 880, move position 40 30' >"$OUT/i3_resize.json" 2>&1 || true
          ;;
        pre_move)
          i3-msg -s "$OUT/i3.sock" 'move position 210 130' >"$OUT/i3_move.json" 2>&1 || true
          ;;
        pure_moved)
          R_MV=$(last_kicad_report "$OUT/client.log" "moved_pre_popup")
          read -r KPID RX RY PW PH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); sr=r.get("shown_root") or {}; p=r.get("pane") or {}; print(int(r.get("pid") or 0), int(sr.get("x",800)), int(sr.get("y",400)), int(p.get("w",600)), int(p.get("h",500)))' "$R_MV")
          if [ "$KPID" -gt 0 ]; then
            track_pid "$KPID"
          fi
          # Aim inside the right-hand GAL board canvas (past KiCad's left Properties pane): left-click to focus, then right-click for menu
          GAL_X=$((RX + PW * 3 / 4))
          GAL_Y=$((RY + PH / 2))
          python3 "$XINPUT_PY" click "$GAL_X" "$GAL_Y" 1
          sleep 0.25
          python3 "$XINPUT_PY" click "$GAL_X" "$GAL_Y" 3
          sleep 0.65
          shot "04_pure_moved_popup"
          if [ "$KPID" -gt 0 ]; then
            python3 "$DRIVER_PY" list-windows --pid "$KPID" --popup >"$OUT/native_popups.json" 2>/dev/null || echo "[]" >"$OUT/native_popups.json"
          else
            echo "[]" >"$OUT/native_popups.json"
          fi
          if [ "$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$OUT/native_popups.json")" -ge 1 ]; then
            python3 "$XINPUT_PY" key Escape
            sleep 0.15
          fi
          python3 "$XINPUT_PY" chord ctrl grave
          sleep 0.2
          ;;
        agent_commit)
          # As an agent thread's kicad_ipc would: its own client name, one commit.
          R_AG=$(last_kicad_report "$OUT/client.log" "warm_pcb_2")
          AG_SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock") or "-")' "$R_AG")
          [ "$AG_SOCK" = "-" ] && AG_SOCK="$API_SOCK"
          BP_IPC_CLIENT="agent:twindow" python3 "$IPC_PY" agent-add-track "$AG_SOCK" 28.0 94.0 155.0 132.0 0.5 3 \
            >"$OUT/agent_commit.json" 2>"$OUT/agent_commit.err" || echo '{"ok": false}' >"$OUT/agent_commit.json"
          sleep 1.2
          shot "12_agent_commit"
          ;;
        *)
          shot "$m"
          ;;
      esac
    fi
  done
  sleep 0.12
done

# In stock/fork mode, verify warm KiCad pid survived ui:View and then close cleanly via hub RPC
WARM_ALIVE_AFTER_VIEW=0
CLOSED_CLEANLY=0
if [ "$EMBED_MODE" != "spike" ]; then
  R_SHOWN=$(last_kicad_report "$OUT/client.log" "shown")
  KPID=$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1]).get("pid") or 0))' "$R_SHOWN")
  if [ "$KPID" -gt 0 ]; then
    track_pid "$KPID"
    if kill -0 "$KPID" 2>/dev/null; then
      WARM_ALIVE_AFTER_VIEW=1
    fi
    CLOSE_OUT=$(hub_rpc "[{\"m\":\"editor.kicad.close\",\"p\":{\"project\":\"$OUT/proj/${KEMBED_LAYOUT:+boards/ecc83-pp/}ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" || true)
    echo "$CLOSE_OUT" >"$OUT/close_rpc.log"
    for _ in $(seq 30); do
      if ! kill -0 "$KPID" 2>/dev/null; then
        CLOSED_CLEANLY=1
        break
      fi
      sleep 0.2
    done
  fi
fi

if [ "${KP:-}" != "" ]; then
  stop_pid "$KP"
fi

# Evaluate assertions across spike or stock/fork reports
python3 - "$OUT" "$EMBED_MODE" "$WARM_ALIVE_AFTER_VIEW" "$CLOSED_CLEANLY" <<'PY'
import json, os, sys

out_dir, mode, warm_alive, closed_clean = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])

ok = fail = 0
def check(what, cond, detail=""):
    global ok, fail
    if cond:
        print(f"PASS {what}")
        ok += 1
    else:
        print(f"FAIL {what}" + (f" ({str(detail)[:320]})" if detail != "" else ""))
        fail += 1

if mode == "spike":
    reps = {}
    with open(os.path.join(out_dir, "spike_reports.jsonl"), encoding="utf-8") as f:
        for line in f:
            if line.strip():
                r = json.loads(line)
                reps[r["mark"]] = r

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
        "Window manager resize (1600x1000 -> 1480x880) resizes embedded KiCad cleanly",
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
        "GPUI overlay punches XShape bounding hole in container and restores on close",
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
else:
    reps = {}
    with open(os.path.join(out_dir, "client.log"), encoding="utf-8", errors="replace") as f:
        for line in f:
            if "GPUI_METRIC " not in line:
                continue
            try:
                obj = json.loads(line.split("GPUI_METRIC ", 1)[1])
            except Exception:
                continue
            if obj.get("event") == "kicad" and obj.get("tag"):
                reps[obj["tag"]] = obj

    def shown_frame(r):
        sx = r.get("shown")
        for fr in r.get("frames") or []:
            if fr.get("xid") == sx:
                return fr
        return {}

    r_shown = reps.get("shown", {})
    f_shown = shown_frame(r_shown)
    c_shown = r_shown.get("container") or {}
    p_shown = r_shown.get("pane") or {}

    check(
        f"Hub spawned KiCad in {mode} mode and reached ready/starting with valid pid",
        r_shown.get("mode") == mode and (r_shown.get("pid") or 0) > 0,
        {"mode": r_shown.get("mode"), "pid": r_shown.get("pid"), "state": r_shown.get("state")},
    )
    check(
        "32-bit GPUI host uses 24-bit DefaultVisual container without BadMatch",
        r_shown.get("toplevel_depth") == 32 and c_shown.get("depth") == 24,
        (r_shown.get("toplevel_depth"), c_shown.get("depth")),
    )
    check(
        "PCB Editor window is reparented inside Backplane GPUI X11 tree",
        r_shown.get("in_gpui_tree") is True and f_shown.get("shown") is True and f_shown.get("kind") == "PCB",
        {"in_gpui_tree": r_shown.get("in_gpui_tree"), "frame": f_shown},
    )
    check(
        "Client-placed PCB Editor matches viewer pane geometry within <=2px",
        abs(c_shown.get("x", -99) - p_shown.get("x", 0)) <= 2
        and abs(c_shown.get("y", -99) - p_shown.get("y", 0)) <= 2
        and abs(f_shown.get("w", -99) - p_shown.get("w", 0)) <= 2
        and abs(f_shown.get("h", -99) - p_shown.get("h", 0)) <= 2,
        {"container": c_shown, "frame": f_shown, "pane": p_shown},
    )
    check(
        "PCB Editor OpenGL GAL canvas painted non-blank content inside Backplane pane",
        (f_shown.get("painted") or 0) > 20 and (c_shown.get("painted") or 0) > 20,
        {"frame_painted": f_shown.get("painted"), "container_painted": c_shown.get("painted")},
    )
    check(
        "Opening embedded KiCad did not steal keyboard focus before user click",
        r_shown.get("keys") is False,
        {"keys": r_shown.get("keys")},
    )

    r_cf = reps.get("clicked_focus", {})
    r_fr = reps.get("focus_returned", {})
    check(
        "Clicking inside embedded PCB canvas gives KiCad focus and Ctrl+` returns focus to GPUI",
        r_cf.get("keys") is True and r_fr.get("keys") is False,
        {"after_click": r_cf.get("keys"), "after_ctrl_backtick": r_fr.get("keys")},
    )

    r_fold = reps.get("sidebar_folded", {})
    c_fold = r_fold.get("container") or {}
    f_fold = shown_frame(r_fold)
    p_fold = r_fold.get("pane") or {}
    check(
        "Folding GPUI sidebar (Ctrl+B) reflows and widens client-placed KiCad pane",
        f_fold.get("shown") is True
        and c_fold.get("x", 9999) < c_shown.get("x", 0)
        and f_fold.get("w", 0) > f_shown.get("w", 0)
        and abs(f_fold.get("w", -99) - p_fold.get("w", 0)) <= 2,
        {"before_x": c_shown.get("x"), "after_x": c_fold.get("x"), "before_w": f_shown.get("w"), "after_w": f_fold.get("w"), "pane_w": p_fold.get("w")},
    )

    r_res = reps.get("resized", {})
    f_res = shown_frame(r_res)
    p_res = r_res.get("pane") or {}
    check(
        "Window manager resize (1800x1050 -> 1480x880) resizes client-placed KiCad cleanly",
        f_res.get("shown") is True
        and f_res.get("w", 9999) < f_shown.get("w", 0)
        and abs(f_res.get("w", -99) - p_res.get("w", 0)) <= 2
        and abs(f_res.get("h", -99) - p_res.get("h", 0)) <= 2,
        {"before_w": f_shown.get("w"), "after_w": f_res.get("w"), "pane": p_res},
    )

    r_mv = reps.get("moved_pre_popup", {})
    sr_mv = r_mv.get("shown_root") or {}
    p_mv = r_mv.get("pane") or {}
    popups = []
    pop_path = os.path.join(out_dir, "native_popups.json")
    if os.path.isfile(pop_path):
        with open(pop_path, encoding="utf-8") as pf:
            popups = json.load(pf)
    exp_rx, exp_ry = sr_mv.get("x", 0), sr_mv.get("y", 0)
    popup_ok = any(
        (exp_rx - 40) <= p.get("root_x", -999) <= (exp_rx + p_mv.get("w", 0) + 40)
        and (exp_ry - 40) <= p.get("root_y", -999) <= (exp_ry + p_mv.get("h", 0) + 40)
        for p in popups
    )
    check(
        "Pure top-level window move + client sync_root anchors right-click menu at new root coords",
        len(popups) >= 1 and popup_ok,
        {"shown_root": sr_mv, "pane": p_mv, "popups": popups},
    )

    r_cut = reps.get("overlay_cutout", {})
    r_cut_closed = reps.get("overlay_closed", {})
    cut_rects = (r_cut.get("container") or {}).get("shape_rects", 0)
    closed_rects = (r_cut_closed.get("container") or {}).get("shape_rects", 0)
    check(
        "GPUI modal overlay (Keysheet F1) punches XShape bounding hole in container and restores on close",
        cut_rects >= 2 and closed_rects == 1,
        {"cut_rects": cut_rects, "closed_rects": closed_rects},
    )

    r_other = reps.get("other_view", {})
    r_back = reps.get("back_board", {})
    check(
        "Switching to non-CAD rail view (Files) unmaps container and switching back restores it",
        (r_other.get("container") or {}).get("shown") is False
        and (r_back.get("container") or {}).get("shown") is True
        and shown_frame(r_back).get("shown") is True,
        {"other_container": r_other.get("container"), "back_container": r_back.get("container")},
    )

    r_csch = reps.get("cold_sch", {})
    r_c3d = reps.get("cold_3d", {})
    r_wsch = reps.get("warm_sch", {})
    r_w3d = reps.get("warm_3d", {})
    r_wpcb = reps.get("warm_pcb_2", {})
    f_wsch = shown_frame(r_wsch)
    f_w3d = shown_frame(r_w3d)
    f_wpcb = shown_frame(r_wpcb)
    check(
        "Rail multi-frame switching (Board <-> Schematic <-> 3D) paints all 3 editors with fast warm switch",
        (f_wsch.get("painted") or 0) > 15
        and (f_w3d.get("painted") or 0) > 15
        and (f_wpcb.get("painted") or 0) > 15
        and (r_wsch.get("wait_ms") or 9999) < 250
        and (r_w3d.get("wait_ms") or 9999) < 250
        and (r_wpcb.get("wait_ms") or 9999) < 250,
        {
            "cold_sch_ms": r_csch.get("wait_ms"),
            "cold_3d_ms": r_c3d.get("wait_ms"),
            "warm_sch_ms": r_wsch.get("wait_ms"),
            "warm_3d_ms": r_w3d.get("wait_ms"),
            "warm_pcb_ms": r_wpcb.get("wait_ms"),
            "sch_painted": f_wsch.get("painted"),
            "v3d_painted": f_w3d.get("painted"),
            "pcb_painted": f_wpcb.get("painted"),
        },
    )

    if mode != "spike":
        try:
            agent_commit = json.load(open(os.path.join(out_dir, "agent_commit.json")))
        except (OSError, ValueError):
            agent_commit = {}
        r_awsch = reps.get("agent_warm_sch", {})
        r_aw3d = reps.get("agent_warm_3d", {})
        r_awpcb = reps.get("agent_warm_pcb", {})
        f_awpcb = shown_frame(r_awpcb)
        check(
            "After a real agent (agent:*) commit opens KiCad's Agents pane, switching tabs still repaints the PCB at once",
            agent_commit.get("ok") is True
            and (f_awpcb.get("painted") or 0) > 15
            and (r_awsch.get("wait_ms") or 9999) < 250
            and (r_aw3d.get("wait_ms") or 9999) < 250
            and (r_awpcb.get("wait_ms") or 9999) < 250,
            {
                "agent_commit": agent_commit,
                "warm_sch_ms": r_awsch.get("wait_ms"),
                "warm_3d_ms": r_aw3d.get("wait_ms"),
                "warm_pcb_ms": r_awpcb.get("wait_ms"),
                "pcb_painted": f_awpcb.get("painted"),
            },
        )

    r_view = reps.get("view_mode", {})
    check(
        "ui:View detaches editor cleanly while keeping warm KiCad alive, and editor.kicad.close exits it",
        r_view.get("ok") is False
        and r_view.get("error") == "not editing"
        and warm_alive == 1
        and closed_clean == 1,
        {"view_report": r_view, "warm_alive": warm_alive, "closed_clean": closed_clean},
    )

print(f"embed_window_e2e ({mode}): passed={ok} failed={fail} artifacts={out_dir}")
sys.exit(1 if fail else 0)
PY
