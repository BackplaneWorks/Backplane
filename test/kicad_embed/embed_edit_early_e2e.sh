#!/bin/bash
# Edit pressed before the viewer knows its file (bp-win after an update: the first preview's key 8 s in, its paint 15 s; the press at
# 6 s was lost, Carbon, Nov 14) is kept: the button says Opening…, and when the picture names the file the editor opens with no second
# press. The slow first picture is the client's test knob (BACKPLANE_GPUI_TEST_KEY_HOLD_MS: no file key, nothing to edit, for its first
# N ms). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_edit_early_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 1500}, {"click": "rail:Board"}, {"wait": 800}, {"click": "ui:Edit"}, {"mark": "pressed"}, {"wait": 1500},
     {"kicad": {"report": True, "tag": "held"}},
     # the hold ends at 9 s; the next picture (here the rail's Board pressed again, as the person's next look) names the file
     {"wait": 7000}, {"click": "rail:Board"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "edit"}}, {"mark": "done"}, {"wait": 1500}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_TEST_KEY_HOLD_MS=9000 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 1100); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" edit)
check "the press was found and kept (no file yet)" "$(grep -q '"id":"ui:Edit","found":false' "$OUT/client.log" && echo 0 || (grep -q '"kicad_edit":"pending"' "$OUT/client.log" && echo 1 || echo 0))"
check "the editor came up with no second press" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(r.get("state")=="ready"))' "$R")" "$(echo "$R" | cut -c1-200)"
check "one Edit press in the script" "$([ "$(grep -c '"ui:Edit"' "$OUT/script.json")" = 1 ] && echo 1 || echo 0)"
wait "$CP" 2>/dev/null || true
finish_suite "embed_edit_early_e2e ($EMBED_MODE)"
