#!/bin/bash
# Diagnosis: Edit on a copy of a real board, then Schematic, with a screenshot and the X window list every KEMBED_WATCH_S seconds
# (what is on screen when a step hangs: a dialog, a blank frame). KEMBED_PROJ_SRC must be a staged copy (lib/real_use.sh stage_board).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
cat >"$OUT/script.json" <<'J'
[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":60000,"kind":"PCB","tag":"cold"}},{"wait":3000},
 {"click":"kicad-tab:SCH"},{"kicad":{"wait_painted":60000,"kind":"SCH","tag":"sch"}},{"wait":2000},{"kicad":{"report":true,"tag":"end"}}]
J
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 150 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
i=0
while kill -0 "$CP" 2>/dev/null; do
  sleep "${KEMBED_WATCH_S:-4}"; i=$((i + 1))
  shot "w$(printf %03d $i)"
  grep -q '"tag":"end"' "$OUT/client.log" 2>/dev/null && break
done
wait "$CP" 2>/dev/null || true
grep -h '"event":"kicad"' "$OUT/client.log" | python3 -c '
import json,sys
for l in sys.stdin:
    r=json.loads(l[l.index("{"):]); print(r.get("tag"), r.get("wait_ms"), r.get("state"), r.get("reason"), [(f.get("kind"),f.get("shown"),f.get("painted"),f.get("w"),f.get("h"),f.get("title")) for f in r.get("frames",[])])'
echo "shots: $OUT/shots"
