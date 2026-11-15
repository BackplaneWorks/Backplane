#!/bin/bash
# The warm first Edit's hand-over (KicadPane::handoff: the viewer's picture kept under a pre-warmed KiCad's frame until it has drawn) never
# stands between the person and the editor: held long (the client's test knob BACKPLANE_GPUI_TEST_HANDOFF_MIN_MS), with KiCad's frame
# mapped over the picture, a right click in the pane opens KiCad's own menu (the input is KiCad's, not the picture's) and Ctrl+K pressed
# then comes back from KiCad (the keyboard was handed in at the placement, not at the hold's end); the hold then ends by itself.
# The blank-frame budget itself is measured on the GPU display (real_use_journeys, first_edit_pixels.blank_frames, strict). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_handoff_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
HOLD_MS=6000
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"mark": "shown"}, {"wait": 20000}, {"click": "ui:Edit"}, {"mark": "edit"}, {"wait": 600},
     {"kicad": {"report": True, "tag": "hold"}}, {"mark": "hold"}, {"wait": 11000}, {"kicad": {"report": True, "tag": "after"}}, {"mark": "done"}, {"wait": 1500}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_TEST_HANDOFF_MIN_MS=$HOLD_MS BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 150 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 600); do grep -q '"name":"edit","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
check "KiCad was warm before Edit (started by the shown thread)" "$(grep -q 'kicad embed: start .*ready' "$OUT/hub.log" && echo 1 || echo 0)"
for _ in $(seq 100); do grep -q '"name":"hold","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.05; done
R=$(last_kicad_report "$OUT/client.log" hold)
read -r KP SHOWN X Y W H < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("pid") or 0, r.get("shown") or 0, f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
check "in the hold, KiCad's frame is placed over the picture" "$([ "$SHOWN" != 0 ] && [ "$W" -gt 100 ] && echo 1 || echo 0)" "$(echo "$R" | cut -c1-200)"
check "the hold is still on (the picture under KiCad's frame)" "$(grep -q '"kicad_handoff_end_ms"' "$OUT/client.log" && echo 0 || echo 1)"
# a right click in the pane: KiCad's own menu opens
python3 "$KEMBED_LIB/rt_input.py" click $((X + W / 2)) $((Y + H / 2)) 3 >/dev/null; sleep 0.6
POP=$(python3 "$DRIVER_PY" list-windows --pid "$KP" --popup 2>/dev/null | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))' 2>/dev/null || echo 0)
check "a right click in the hold opens KiCad's menu (the input is KiCad's)" "$([ "${POP:-0}" -ge 1 ] && echo 1 || echo 0)" "popups=$POP"
python3 "$XINPUT_PY" key Escape; sleep 0.3
N0=$(grep -c '"kicad_ev":"key ctrl-k"' "$OUT/client.log" || true)
python3 "$XINPUT_PY" chord ctrl k; sleep 0.6
N1=$(grep -c '"kicad_ev":"key ctrl-k"' "$OUT/client.log" || true)
check "Ctrl+K in the hold comes back from KiCad (the keyboard was handed in at once)" "$([ "${N1:-0}" -gt "${N0:-0}" ] && echo 1 || echo 0)"
check "all that inside the hold" "$(grep -q '"kicad_handoff_end_ms"' "$OUT/client.log" && echo 0 || echo 1)"
for _ in $(seq 200); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
E=$(grep -o '"kicad_handoff_end_ms":[0-9]*,"placed_ms":[0-9a-z]*,"drawn_event":[a-z]*' "$OUT/client.log" | head -1 || true)
echo "INFO hand-over: $E"
check "the hold ends by itself" "$([ -n "$E" ] && echo 1 || echo 0)"
A=$(last_kicad_report "$OUT/client.log" after)
check "the editor is up after it" "$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1] or "{}").get("state")=="ready"))' "$A")"
wait "$CP" 2>/dev/null || true
finish_suite "embed_handoff_e2e ($EMBED_MODE)"
