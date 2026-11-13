#!/bin/bash
# Suite 9: Backplane's own Gerber viewer (native/host/src/kicad/gerber, the Board page drawing a set).
#   G1  the project's mfg/ output (exported here from the fixture with the runtime's kicad-cli) opens on the rail's Gerbers tab: the
#       hub watches the set, the plot paints (chunks), the rail lights Gerbers
#   G2  Board, then Gerbers again: painted within 300 ms of the press (the set read once)
#   G3  a Gerber file opened from the Files tab is drawn itself
# GERBERS_BOARD=<a copy of a project folder with mfg/> uses that project instead (measurement and screenshots on a real board).
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
  [ -x "$KICAD_CLI" ] || { echo "SKIP embed_gerbers_e2e: no kicad-cli to export the fixture's Gerbers"; exit 0; }
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
     {"click": "rail:Board"}, {"wait": 4000}, {"mark": "board"}, {"wait": 2000}, {"click": "rail:Gerbers"}, {"wait": 4000}, {"mark": "gerbers2"}, {"wait": 2000}, {"layers": True}, {"wait": 2500}, {"mark": "layers"}, {"wait": 2500}, {"layers": False}, {"wait": 1000},
     {"click": "rail:Files"}, {"wait": 2500}, *[x for part in sys.argv[2].split("/") for x in ({"files": "filter:" + part}, {"wait": 1200}, {"files": "open:0"}, {"wait": 1500})], {"wait": 2500}, {"mark": "files"}, {"wait": 60000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 >"$OUT/client.log" 2>&1 &
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
wait_for_log "$OUT/client.log" '"name":"files","event":"mark"' 200 || true
shot "g3_files"
check "A Gerber opened from the Files tab is drawn itself (its key names the file)" "$(grep -q "\"key\":\"0|$PROJ|$PROJ/$SET_FILE\"" "$OUT/client.log" && echo 1 || echo 0)"
grep -h "plotnote" "$OUT/client.log" | tail -2 || true
finish_suite "embed_gerbers_e2e ($EMBED_MODE)"
