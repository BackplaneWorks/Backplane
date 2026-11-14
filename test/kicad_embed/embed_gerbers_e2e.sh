#!/bin/bash
# Suite 9: Backplane's own Gerber viewer (native/host/src/kicad/gerber, the Board page drawing a set).
#   G1  the project's mfg/ output (exported here from the fixture with the runtime's kicad-cli) opens on the rail's Gerbers tab: the
#       hub watches the set, the plot paints (chunks), the rail lights Gerbers
#   G2  Board, then Gerbers again: painted within 300 ms of the press (the set read once)
#   G3  a Gerber file opened from the Files tab is drawn itself
# GERBERS_BOARD=<a copy of a project folder with mfg/> uses that project instead (measurement and screenshots on a real board);
# GERBERS_THEME=light|dark draws the client in that theme.
# Copies only (seed_project.py), private Xvfb + i3. EMBED_MODE=stock|fork (no KiCad editor is opened; kicad-cli exports the fixture).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
if [ "$EMBED_MODE" = "spike" ]; then echo "SKIP embed_gerbers_e2e: product modes only (EMBED_MODE=stock|fork)"; exit 0; fi
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
[ -n "${KICAD_CLI:-}" ] || KICAD_CLI="$(dirname "${KICAD_BIN:-/usr/bin/kicad}")/kicad-cli"
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"
if [ -n "${GERBERS_BOARD:-}" ]; then
  ( cd "$PROJ" && git rm -q -r --cached . >/dev/null && find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} + )
  rsync -a --exclude '*backups*' --exclude '~*.lck' --exclude '.git' "$GERBERS_BOARD"/ "$PROJ"/
  echo "{\"project\": \"$(basename "$(ls "$PROJ"/*.kicad_pro | head -1)")\"}" >"$PROJ/.backplane.json"
else
  [ -x "$KICAD_CLI" ] || { echo "FAIL embed_gerbers_e2e: no kicad-cli to export the fixture's Gerbers (KICAD_CLI=${KICAD_CLI:-unset})"; exit 1; }
  mkdir -p "$PROJ/mfg/gerbers"
  "$KICAD_CLI" pcb export gerbers -o "$PROJ/mfg/gerbers/" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1
  "$KICAD_CLI" pcb export drill --format excellon --excellon-separate-th -o "$PROJ/mfg/gerbers/" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1
fi
( cd "$PROJ" && git add -A >/dev/null && git commit -qm "fab output" >/dev/null ) || true
SET_FILE=$(cd "$PROJ" && find mfg \( -name '*F_Cu*' -o -name '*.gtl' \) | head -1)
echo "a Gerber for the Files tab: $SET_FILE"
start_xvfb_and_wm
start_hub
python3 - "$OUT/script.json" "$SET_FILE" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2500}, {"click": "rail:Gerbers"}, {"wait": 8000}, {"mark": "gerbers"}, {"wait": 3000},
     {"click": "rail:Board"}, {"wait": 4000}, {"mark": "board"}, {"wait": 2000}, {"click": "rail:Gerbers"}, {"wait": 4000}, {"mark": "gerbers2"}, {"wait": 2000}, {"layers": True}, {"wait": 2500}, {"mark": "layers"}, {"wait": 2500}, {"layers": False}, {"wait": 1000}, {"action": ["view", "gerbers\tboard"]}, {"wait": 15000}, {"mark": "vs"}, {"wait": 2500}, {"click": "mgmt:checks"}, {"wait": 3000}, {"click": "mgmt:checks:src:gerbers"}, {"wait": 3000}, {"mark": "checks_open"}, {"wait": 12000}, {"mark": "clicked"}, {"click": "mgmt:checks"}, {"wait": 1500}, {"action": ["view", "gerbers"]}, {"wait": 2000},
     {"click": "rail:Files"}, {"wait": 2500}, *[x for part in sys.argv[2].split("/") for x in ({"files": "filter:" + part}, {"wait": 1200}, {"files": "open:0"}, {"wait": 1500})], {"wait": 2500}, {"mark": "files"}, {"wait": 60000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" ${GERBERS_THEME:+--theme "$GERBERS_THEME"} --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 180 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
# the first event of a name after line $2 (the last when $3 is "last")
ev() { python3 - "$OUT/client.log" "$1" "${2:-0}" "${3:-first}" <<'PY'
import json, sys
log, ev, after, which = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]; found = {}
for i, line in enumerate(open(log, errors="replace")):
    if i < after: continue
    for tag in ("GPUI_LOG ", "GPUI_METRIC "):
        if tag in line:
            try: o = json.loads(line.split(tag, 1)[1])
            except Exception: continue
            if o.get("event") == ev:
                found = o
                if which == "first": print(json.dumps(found)); sys.exit(0)
print(json.dumps(found))
PY
}
lineno() { grep -n "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" | head -1 | cut -d: -f1; }
wait_for_log "$OUT/client.log" '"name":"gerbers","event":"mark"' 120 || { echo "FAIL the client never reached the Gerbers tab"; tail -n 20 "$OUT/client.log"; exit 1; }
P1=$(ev plot_painted)
shot "g1_gerbers"
check "The Gerbers tab drew the project's set ($P1)" "$(python3 -c 'import json,sys; p=json.loads(sys.argv[1]); print(int(p.get("chunks",0) > 0 and "mfg" in p.get("key","")))' "$P1")"
SETDIR=$(dirname "$PROJ/$SET_FILE")
check "The plot is of the mfg/ set (its key names $SETDIR)" "$(grep -q "\"key\":\"0|$PROJ|$SETDIR\"" "$OUT/client.log" && echo 1 || echo 0)"
wait_for_log "$OUT/client.log" '"name":"gerbers2","event":"mark"' 200 || true
L=$(lineno board); P2=$(ev plot_painted "${L:-0}")
check "Gerbers again after Board: the set painted within 300 ms of the press ($P2)" "$(python3 -c 'import json,sys; p=json.loads(sys.argv[1]); print(int(p.get("chunks",0)>0 and p.get("ms",1e9) <= 300 and "mfg" in p.get("key","")))' "$P2")"
shot "g2_gerbers_again"
wait_for_log "$OUT/client.log" '"name":"layers","event":"mark"' 200 || true
shot "g2b_layers"
wait_for_log "$OUT/client.log" '"name":"vs","event":"mark"' 200 || true
shot "g4_vs_board"
check "vs board: the set compared with its board was drawn (its key names the comparison)" "$(grep -q 'gerbercmp' "$OUT/client.log" && echo 1 || echo 0)"
if [ -z "${GERBERS_BOARD:-}" ]; then
  check "A set exported now is the same as its board: no marks ($(grep 'compared with its board' "$OUT/hub.log" | tail -1))" "$(grep -q 'compared with its board in [0-9]* ms (0 marks)' "$OUT/hub.log" && echo 1 || echo 0)"
fi
# each difference is a finding in the Checks panel (source gerbers); a press on one lights and zooms on it in the comparison
PRO=$(ls "$PROJ"/*.kicad_pro | head -1)
NG=$(hub_rpc "[{\"method\":\"kicad.checks.list\",\"params\":{\"project\":\"$PRO\",\"source\":\"gerbers\",\"rows\":true}}]" | grep -v '^INFO ' | head -1 | python3 -c 'import json,sys; r=(json.load(sys.stdin).get("result") or {}); print(r.get("total", -1))' 2>/dev/null || echo -1)
if [ -z "${GERBERS_BOARD:-}" ]; then check "No difference findings for a set exported now ($NG)" "$( [ "$NG" = 0 ] && echo 1 || echo 0)"
else
  check "The differences are findings in the Checks panel ($NG)" "$( [ "$NG" -gt 0 ] 2>/dev/null && echo 1 || echo 0)"
  wait_for_log "$OUT/client.log" '"name":"checks_open","event":"mark"' 200 || true
  read -r GX GY < <(python3 "$DRIVER_PY" list-windows --pid "$CP" | python3 -c 'import json,sys; w=sorted([x for x in json.load(sys.stdin) if x.get("shown")], key=lambda x: -x.get("w",0)*x.get("h",0)); print(w[0]["root_x"], w[0]["root_y"]) if w else print(0, 0)')
  # the rows as drawn: the first is the stale export's one must-change (rule export), the next a named difference
  rows() { python3 - "$OUT/client.log" "$GX" "$GY" "$1" <<'PY'
import json, sys
log, gx, gy, k = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]); rows = []
for line in open(log, errors="replace"):
    if "GPUI_METRIC " not in line or '"controls"' not in line: continue
    try: v = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    got = [c["rect"] for c in v.get("controls", []) if isinstance(c, dict) and c.get("id", "").startswith("mgmt:checks:row:gerbers:") and c.get("rect")]
    if got: rows = got
if len(rows) > k: f = rows[k]; print(int(gx + f["x"] + min(f["w"] / 2, 60)), int(gy + f["y"] + f["h"] / 2))
PY
  }
  XY=$(rows 0); [ -n "$XY" ] && python3 "$XINPUT_PY" click $XY; sleep 3
  shot "g5a_stale"
  XY=$(rows 1)
  L0=$(wc -l <"$OUT/client.log"); [ -n "$XY" ] && python3 "$XINPUT_PY" click $XY; sleep 4
  shot "g5_difference"
  check "A difference row pressed: lit and zoomed on in the comparison" "$(tail -n +"$L0" "$OUT/client.log" | grep -q 'probe_sync.*difference;at=' && tail -n +"$L0" "$OUT/client.log" | grep -q '"xprobe_zoom".*difference;at=' && echo 1 || echo 0)" "row at ${XY:-none}"
  NE=$(hub_rpc "[{\"method\":\"kicad.checks.list\",\"params\":{\"project\":\"$PRO\",\"source\":\"gerbers\",\"severity\":\"error\",\"rows\":true}}]" | grep -v '^INFO ' | head -1 | python3 -c 'import json,sys; r=(json.load(sys.stdin).get("result") or {}); print(r.get("total", -1))' 2>/dev/null || echo -1)
  check "A stale export is one must-change ($NE error of $NG findings)" "$( [ "$NE" = 1 ] && echo 1 || echo 0)"
fi
wait_for_log "$OUT/client.log" '"name":"files","event":"mark"' 200 || true
shot "g3_files"
check "A Gerber opened from the Files tab is drawn itself (its key names the file)" "$(grep -q "\"key\":\"0|$PROJ|$PROJ/$SET_FILE\"" "$OUT/client.log" && echo 1 || echo 0)"
finish_suite "embed_gerbers_e2e ($EMBED_MODE)"
