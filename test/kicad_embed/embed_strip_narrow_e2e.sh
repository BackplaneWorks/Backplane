#!/bin/bash
# The editor's strip at the narrowest pane (380 px, local.rs VIEWER_PX), in the dark and the light theme, with KiCad's long job in it (a zone
# fill asked over IPC) and the checks chip: Full size, Full screen and Pop out are all there and inside the pane; the chip is whole and left of
# them, or not shown at all (never cut). A 1:1 shot of the strip is kept for each theme (KEEP_SHOTS=<dir> copies them out). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# each theme on its own instance (STRIP_THEME picks one)
if [ -z "${STRIP_THEME:-}" ]; then rc=0; for t in dark light; do for w in 1000 700 380; do STRIP_THEME=$t STRIP_W=$w OUT= bash "$0" || rc=1; done; done; exit $rc; fi
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_strip_narrow_e2e: fork only"; exit 0; }
discover_bins; discover_kicad
THEME=$STRIP_THEME; SW=${STRIP_W:-380}
  init_isolated_dirs
  python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
  start_xvfb_and_wm; start_hub
  echo "{\"theme\": \"$THEME\", \"viewer_px\": $SW}" > "$OUT/local-settings.json"
  cat > "$OUT/script.json" <<'J'
[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"up"}},{"wait":6000},{"kicad":{"report":true,"tag":"r"}},{"mark":"ready"},{"wait":2500},{"mark":"job"},{"wait":12000},{"mark":"jobdone"},{"wait":3000},{"mark":"done"}]
J
  HOME="$OUT/fh" BACKPLANE_GPUI_LOCAL="$OUT/local-settings.json" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 >"$OUT/client.log" 2>&1 &
  CP=$!; track_pid "$CP"
  for _ in $(seq 1200); do grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
  R=$(last_kicad_report "$OUT/client.log" r)
  SOCK=$(echo "$R" | python3 -c 'import json,sys; print(json.loads(sys.stdin.read() or "{}").get("sock") or "")')
  read -r PX PY PW PH < <(echo "$R" | python3 -c 'import json,sys; p=json.loads(sys.stdin.read() or "{}").get("pane") or {}; print(p.get("x",0), p.get("y",0), p.get("w",0), p.get("h",0))')
  # KiCad's long jobs while the client's marks "job".."jobdone" run: zone fills back to back (each one a job in the strip's bar), shots meanwhile
  for _ in $(seq 400); do grep -q '"name":"job","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.05; done
  ( for _ in 1 2 3 4 5 6; do python3 "$IPC_PY" run-action "$SOCK" "pcbnew.ZoneFiller.zoneUnfillAll" >/dev/null 2>&1; python3 "$IPC_PY" run-action "$SOCK" "pcbnew.ZoneFiller.zoneFillAll" >/dev/null 2>&1; done ) &
  mkdir -p "$OUT/shots"
  for i in 1 2 3 4 5 6 7 8; do import -window root -crop "$((PW + 4))x34+$((PX - 2))+$((PY - 32))" "$OUT/shots/strip-$THEME-$SW-$i.png" 2>/dev/null || true; sleep 0.8; done
  for _ in $(seq 300); do grep -q '"name":"jobdone","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done
  JOB=$(python3 "$KEMBED_LIB/strip_frames.py" "$OUT/client.log" job jobdone "$PX" "$PW" "$([ "$SW" -ge 900 ] && echo 1 || echo 0)")
  echo "INFO $THEME, pane $SW, during KiCad's jobs: $JOB (ok, frames with the bar, frames with the bar and the chip, faults)"
  check "$THEME, $SW px pane: with KiCad's job bar up, the bar and the checks chip are whole between the tabs and the buttons$([ "$SW" -ge 900 ] && echo ", both shown")" "${JOB%% *}" "$JOB"
  OUTV=$(python3 - "$OUT/client.log" "$PX" "$PW" <<'PY'
import json, sys
log, px, pw = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
last = None
for l in open(log, errors="ignore"):
    if '"event":"probes"' in l: last = json.loads(l[l.index("{"):])
r = {x["id"]: x for x in (last or {}).get("rects", [])}
btn = [r.get(k) for k in ("kicad:full", "kicad:screen", "kicad:popout")]
inside = all(b and b["x"] >= px - 1 and b["x"] + b["w"] <= px + pw + 1 for b in btn)
chip = r.get("kicad:checks-chip"); first = min((b["x"] for b in btn if b), default=1e9); row = btn[0]["y"] if btn[0] else 0
chip_ok = chip is None or chip["y"] > row + 20 or chip["x"] + chip["w"] <= first + 0.5
print(int(inside), int(chip_ok), json.dumps({"pane": [px, pw], "buttons": [b and [b["x"], b["w"]] for b in btn], "chip": chip and [chip["x"], chip["y"], chip["w"]], "progress": "kicad-progress" in json.dumps(last or {})}))
PY
)
  read -r IN CHIP DETAIL <<<"$OUTV"
  echo "INFO $THEME, pane $SW: $DETAIL"
  check "$THEME, $SW px pane: Full size, Full screen and Pop out are all inside the pane" "$IN" "$DETAIL"
  check "$THEME, $SW px pane: the checks chip is whole and left of them, or not shown" "$CHIP" "$DETAIL"
  [ -n "${KEEP_SHOTS:-}" ] && { mkdir -p "$KEEP_SHOTS"; cp "$OUT"/shots/strip-$THEME-$SW-*.png "$KEEP_SHOTS/" 2>/dev/null; }
  kill "$CP" 2>/dev/null || true
finish_suite "embed_strip_narrow_e2e ($EMBED_MODE, $THEME, $SW px)"
