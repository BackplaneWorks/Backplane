#!/bin/bash
# Full size and Full screen, on and off, by their own buttons (kicad:full, kicad:screen: the strip's rects): after each round trip the pane is
# back where it was to the pixel, and KiCad's shown frame is exactly over the pane at every step (no strip of the picture, no gap at an edge).
# 1:1 shots of the pane's edges before and after each round trip are kept in the artifacts. fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_fullsize_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "up"}},
     {"kicad": {"report": True, "tag": "sock"}}, {"mark": "up"}, {"wait": 9000}]
for tag, click in (("before", None), ("full_on", "kicad:full"), ("full_off", "kicad:full"), ("screen_on", "kicad:screen"), ("screen_off", "kicad:screen")):
    if click: s += [{"click": click}, {"wait": 2500}]
    s += [{"kicad": {"report": True, "tag": tag}}, {"mark": tag}, {"wait": 1200}]
s += [{"mark": "done"}, {"wait": 1000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 150 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mkdir -p "$OUT/shots"
# KiCad's long jobs while the editor is up (its start-up work, then a zone fill asked over IPC): their bar in the strip never moves the buttons
for _ in $(seq 1200); do grep -q '"name":"up","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
SOCK=$(last_kicad_report "$OUT/client.log" sock | python3 -c 'import json,sys; print(json.loads(sys.stdin.read() or "{}").get("sock") or "")')
[ -n "$SOCK" ] && { sleep 2; python3 "$IPC_PY" run-action "$SOCK" "pcbnew.ZoneFiller.zoneFillAll" >/dev/null 2>&1 || true; }
for tag in before full_on full_off screen_on screen_off; do
  for _ in $(seq 1200); do grep -q "\"name\":\"$tag\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
  import -window root "$OUT/shots/$tag.png" 2>/dev/null || true
done
geo() {  # the report's pane rect and the shown frame's place on screen: "px py pw ph fx fy fw fh"
  python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); p=r.get("pane") or {}; f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{})
print(p.get("x",-1), p.get("y",-1), p.get("w",-1), p.get("h",-1), f.get("root_x",-1), f.get("root_y",-1), f.get("w",-1), f.get("h",-1))' "$(last_kicad_report "$OUT/client.log" "$1")"; }
for tag in before full_on full_off screen_on screen_off; do
  read -r PX PY PW PH FX FY FW FH < <(geo $tag)
  echo "INFO $tag: pane $PX,$PY ${PW}x$PH; KiCad's frame $FX,$FY ${FW}x$FH"
  check "$tag: KiCad's frame is exactly over the pane" "$([ "$PW" -gt 100 ] && [ "$FX $FY $FW $FH" = "$PX $PY $PW $PH" ] && echo 1 || echo 0)" "pane $PX,$PY ${PW}x$PH, frame $FX,$FY ${FW}x$FH"
done
MOVES=$(python3 "$KEMBED_LIB/strip_moves.py" "$OUT/client.log")
echo "INFO the strip's buttons from Edit to the first toggle: $MOVES (distinct places, snapshots)"
check "KiCad's jobs and the checks chip never move the strip's buttons" "$([ "${MOVES%% *}" = 1 ] && echo 1 || echo 0)" "$MOVES"
B=$(geo before | cut -d' ' -f1-4)
check "Full size on: the pane is bigger" "$(python3 -c 'import sys; a=list(map(int,sys.argv[1].split())); b=list(map(int,sys.argv[2].split())); print(int(b[2]*b[3] > a[2]*a[3]))' "$B" "$(geo full_on | cut -d' ' -f1-4)")"
check "Full size off: the pane is back where it was, to the pixel" "$([ "$(geo full_off | cut -d' ' -f1-4)" = "$B" ] && echo 1 || echo 0)" "before $B, after $(geo full_off | cut -d' ' -f1-4)"
check "Full screen on: the pane is bigger" "$(python3 -c 'import sys; a=list(map(int,sys.argv[1].split())); b=list(map(int,sys.argv[2].split())); print(int(b[2]*b[3] > a[2]*a[3]))' "$B" "$(geo screen_on | cut -d' ' -f1-4)")"
check "Full screen off: the pane is back where it was, to the pixel" "$([ "$(geo screen_off | cut -d' ' -f1-4)" = "$B" ] && echo 1 || echo 0)" "before $B, after $(geo screen_off | cut -d' ' -f1-4)"
# 1:1 shots of the pane's four edges (8 px either side) before and after each round trip, for a look
read -r PX PY PW PH _ < <(geo before)
for tag in before full_off screen_off; do
  [ -f "$OUT/shots/$tag.png" ] || continue
  convert "$OUT/shots/$tag.png" -crop "$((PW + 16))x16+$((PX - 8))+$((PY - 8))" "$OUT/shots/${tag}_top.png" 2>/dev/null || true
  convert "$OUT/shots/$tag.png" -crop "16x$((PH + 16))+$((PX + PW - 8))+$((PY - 8))" "$OUT/shots/${tag}_right.png" 2>/dev/null || true
done
for _ in $(seq 100); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done
kill "$CP" 2>/dev/null || true
[ -n "${KEEP_SHOTS:-}" ] && { mkdir -p "$KEEP_SHOTS"; cp "$OUT"/shots/*.png "$KEEP_SHOTS"/ 2>/dev/null || true; }
finish_suite "embed_fullsize_e2e ($EMBED_MODE)"
