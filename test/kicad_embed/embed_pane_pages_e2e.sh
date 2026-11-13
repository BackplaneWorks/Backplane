#!/bin/bash
# Suite: the pages that share the pane with the embedded KiCad (screens/viewer.rs): with Edit on and KiCad's PCB editor in the pane, a press
# on a page that is not KiCad's (Gerbers, Split, Mech, Files, Review, Browser) takes KiCad's windows off the pane (its container unmapped,
# KiCad kept attached) and that page is drawn; Board brings the editor back at once. Gerbers is the Board page drawing a set, which read as
# KiCad's page while Edit was on (lane K's rerun 9).
#   P1  Edit: KiCad's PCB editor mapped in the pane
#   P2  each other page: KiCad's container unmapped (a screenshot each); Gerbers: the set's plot painted (lane K's rerun 9 bug: the rail
#       lit Gerbers, the PCB editor stayed on screen)
#   P3  Board again: the editor mapped in the pane, within 1 s, the same KiCad (no new attach)
# Copies only (seed_project.py; the fixture's Gerbers exported by kicad-cli into mfg/). EMBED_MODE=fork|stock.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
[ "$EMBED_MODE" = "spike" ] && { echo "SKIP embed_pane_pages_e2e: product modes only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
[ -n "${KICAD_CLI:-}" ] || KICAD_CLI="$(dirname "${KICAD_BIN:-/usr/bin/kicad}")/kicad-cli"
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots" "$PROJ/mfg/gerbers"
"$KICAD_CLI" pcb export gerbers -o "$PROJ/mfg/gerbers/" "$PCB" >/dev/null 2>&1 || true
"$KICAD_CLI" pcb export drill --format excellon -o "$PROJ/mfg/gerbers/" "$PCB" >/dev/null 2>&1 || true
git -C "$PROJ" add -A >/dev/null 2>&1; git -C "$PROJ" commit -qm "fab output" >/dev/null 2>&1 || true
start_xvfb_and_wm
start_hub
PAGES="Gerbers Split Mech Files Review Browser"
python3 - "$OUT/script.json" "$OUT/shots" $PAGES <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2000}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"},
     {"kicad": {"wait_painted": 60000, "tag": "painted"}}, {"wait": 2000}, {"click": "kicad-tab:PCB"}, {"wait": 3000},
     {"kicad": {"report": True, "tag": "edit"}}, {"shot": f"{sys.argv[2]}/p1_edit.png"}]
for p in sys.argv[3:]:
    t = p.lower()
    s += [{"click": f"rail:{p}"}, {"wait": 2500}, {"kicad": {"report": True, "tag": t}}, {"shot": f"{sys.argv[2]}/p2_{t}.png"},
          {"click": "rail:Board"}, {"mark": f"back_{t}"}, {"wait": 1000}, {"kicad": {"report": True, "tag": f"back_{t}"}}, {"shot": f"{sys.argv[2]}/p3_back_{t}.png"}]
s += [{"wait": 1000}, {"mark": "done"}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 600 || { echo "FAIL the script never finished"; tail -n 20 "$OUT/client.log"; finish_suite "embed_pane_pages_e2e ($EMBED_MODE)"; exit 1; }
rep() { last_kicad_report "$OUT/client.log" "$1"; }
mapped() { python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); c=r.get("container") or {}; print(1 if c.get("shown") else 0)' "$1" 2>/dev/null || echo 0; }
E=$(rep edit); PID0=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("pid") or "")' "$E" 2>/dev/null)
check "Edit: KiCad's PCB editor mapped in the pane (P1)" "$(mapped "$E")" "$(echo "$E" | cut -c1-300)"
for p in $PAGES; do
  t=$(echo "$p" | tr 'A-Z' 'a-z')
  R=$(rep "$t"); B=$(rep "back_$t")
  check "$p with Edit on: KiCad's windows off the pane (container unmapped, KiCad still attached)" "$([ "$(mapped "$R")" = 0 ] && [ -n "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("pid") or "")' "$R" 2>/dev/null)" ] && echo 1 || echo 0)" "$(echo "$R" | cut -c1-300); see shots/p2_$t.png"
  check "Board after $p: the editor back in the pane within 1 s, the same KiCad" "$([ "$(mapped "$B")" = 1 ] && [ "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("pid") or "")' "$B" 2>/dev/null)" = "$PID0" ] && echo 1 || echo 0)" "$(echo "$B" | cut -c1-300)"
done
# the Gerbers page drew its set: a plot painted between the press on Gerbers and the next on Board (lane K's rerun 9: none)
GP=$(python3 - "$OUT/client.log" <<'PY'
import sys
lines = open(sys.argv[1], errors="ignore").read().splitlines()
i = next((n for n, l in enumerate(lines) if '"id":"rail:Gerbers"' in l and '"event":"click"' in l), None)
if i is None: print(0); sys.exit()
j = next((n for n in range(i + 1, len(lines)) if '"id":"rail:Board"' in lines[n] and '"event":"click"' in lines[n]), len(lines))
print(sum(1 for l in lines[i:j] if '"event":"plot_painted"' in l))
PY
)
check "Gerbers with Edit on: the set's plot painted in the pane ($GP paints between the press and Board)" "$([ "${GP:-0}" -gt 0 ] && echo 1 || echo 0)" "see shots/p2_gerbers.png"
finish_suite "embed_pane_pages_e2e ($EMBED_MODE)"
