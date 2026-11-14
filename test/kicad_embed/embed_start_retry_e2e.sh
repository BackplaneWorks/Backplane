#!/bin/bash
# A KiCad start that misses its window (Windows' first start after an install or update, the antivirus reading the new files: Carbon,
# Nov 13) is tried once more by the hub (Lane K's 08611646a) before anything is said: once stalled, Edit still shows the editor and the pane never says KiCad
# could not start; twice stalled, the pane says so with Try again, and Try again starts it. The stall is the hub's test knob
# (BACKPLANE_KICAD_TEST_STALLS: its first N starts never see KiCad's socket) in a short window (BACKPLANE_KICAD_START_WINDOW_S). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# both cases, each on its own instance (RETRY_STALLS picks one)
if [ -z "${RETRY_STALLS:-}" ]; then
  rc=0; for n in 1 2; do RETRY_STALLS=$n OUT= bash "$0" || rc=1; done; exit $rc
fi
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_start_retry_e2e: fork only"; exit 0; }
discover_bins; discover_kicad
export BACKPLANE_KICAD_WARM=0 BACKPLANE_KICAD_START_WINDOW_S=${BACKPLANE_KICAD_START_WINDOW_S:-8}
one() {  # one() STALLS: a fresh instance whose first STALLS KiCad starts stall
  local stalls=$1
  init_isolated_dirs
  python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
  start_xvfb_and_wm
  BACKPLANE_KICAD_TEST_STALLS=$stalls start_hub
  python3 - "$OUT/script.json" "$stalls" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}]
if sys.argv[2] == "1":
    s += [{"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "edit"}}]
else:
    # both starts stall: the failure is said after two windows; then Try again
    s += [{"wait": 23000}, {"kicad": {"report": True, "tag": "failed"}}, {"click": "kicad:retry"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "retried"}}]
s += [{"mark": "done"}, {"wait": 1500}]
json.dump(s, open(sys.argv[1], "w"))
PY
  HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 150 >"$OUT/client.log" 2>&1 &
  local cp=$!; track_pid "$cp"
  for _ in $(seq 1400); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$cp" 2>/dev/null || break; sleep 0.1; done
  grep -E "kicad embed: (start|started)" "$OUT/hub.log" | sed "s/^/INFO /" || true
  if [ "$stalls" = 1 ]; then
    local r; r=$(last_kicad_report "$OUT/client.log" edit)
    check "one stalled start: the hub tried once more by itself" "$(grep -q 'starting it once more' "$OUT/hub.log" && grep -q 'started after a missed start window' "$OUT/hub.log" && echo 1 || echo 0)"
    check "one stalled start: Edit shows the editor" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(r.get("state")=="ready" and (r.get("wait_ms") or -1) >= 0))' "$r")" "$(echo "$r" | cut -c1-200)"
    check "one stalled start: the pane never said KiCad could not start" "$(grep -q '"state":"failed"' "$OUT/client.log" && echo 0 || echo 1)"
  else
    local f r; f=$(last_kicad_report "$OUT/client.log" failed); r=$(last_kicad_report "$OUT/client.log" retried)
    check "two stalled starts: the hub retried once, then the pane says KiCad could not start" "$(python3 -c 'import json,sys; s=json.loads(sys.argv[1] or "{}").get("say") or ""; print(int("could not start" in s))' "$f")$( [ "$(grep -c 'starting it once more' "$OUT/hub.log")" = 1 ] && echo '' || echo x)" "$(echo "$f" | python3 -c 'import json,sys; print((json.load(sys.stdin).get("say") or "")[:240])' 2>/dev/null)"
    check "two stalled starts: the pane offers Try again" "$(grep -q '"id":"kicad:retry","found":true\|"click:kicad:retry"\|"step":"click:kicad:retry"' "$OUT/client.log" && echo 1 || (grep -q '"id":"kicad:retry","found":false' "$OUT/client.log" && echo 0 || echo 1))"
    check "two stalled starts: Try again starts KiCad and shows the editor" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(r.get("state")=="ready"))' "$r")" "$(echo "$r" | cut -c1-200)"
  fi
  wait "$cp" 2>/dev/null || true
}
one "$RETRY_STALLS"
finish_suite "embed_start_retry_e2e ($EMBED_MODE, $RETRY_STALLS stalled)"
