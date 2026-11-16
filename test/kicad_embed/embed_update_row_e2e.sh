#!/bin/bash
# The Backplane KiCad update at Edit is a row of its own above the KiCad pane, never over the pane's area (on macOS KiCad's frame is
# another process's window above Backplane's: anything drawn over the pane is hidden under it). An offer is given to the client at its
# start (BACKPLANE_GPUI_KICAD_UPDATE, the hub's kicad.update as lane K's hub publishes it), then Edit:
#   U1  the row (kicad-update) and its Update and Not now are drawn, with the offer's words
#   U2  the row's rect does not intersect the pane's (kicad:pane), nor does any of its buttons'
#   U3  "Not now" puts it away and keeps that version on this device (local settings), the pane still there
# fork only; under i3 and mutter (KEMBED_WM).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_update_row_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
echo '{}' > "$OUT/local-settings.json"
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"wait":3000},{"mark":"ready"},{"wait":30000},{"mark":"done"}]' >"$OUT/script.json"
OFFER='{"offer":true,"from":"10.99.0-bp.12.4","to":"10.99.0-bp.13","reason":"Backplane KiCad bp.13 is out: the editor fits a narrow pane","mb":1083,"size":"about 1.1 GB"}'
HOME="$OUT/fh" BACKPLANE_GPUI_KICAD_UPDATE="$OFFER" BACKPLANE_GPUI_LOCAL="$OUT/local-settings.json" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 900 || { echo "FAIL the board never opened in KiCad"; exit 1; }
# the latest drawn rects of the row, its buttons and the pane (window coordinates)
rects() { python3 - "$OUT/client.log" <<'PY'
import json, sys
last = {}
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"rects":[' in l:
        try: rs = json.loads(l.split("GPUI_METRIC ", 1)[1])["rects"]
        except Exception: continue
        if isinstance(rs, list) and any(r.get("id", "").startswith(("kicad:pane", "kicad-update")) for r in rs):
            for r in rs: last[r["id"]] = r
            last["_ids"] = [r["id"] for r in rs]
print(json.dumps(last))
PY
}
R=$(rects)
U=$(python3 -c 'import json,sys
r=json.loads(sys.argv[1]); ids=set(r.get("_ids",[]))
pane=r.get("kicad:pane"); row=r.get("kicad-update")
def hit(a,b): return a and b and a["x"] < b["x"]+b["w"] and b["x"] < a["x"]+a["w"] and a["y"] < b["y"]+b["h"] and b["y"] < a["y"]+a["h"]
shown=int(bool(row) and "kicad-update" in ids and "kicad-update:go" in ids and "kicad-update:later" in ids)
apart=int(bool(row and pane) and not any(hit(r.get(k), pane) for k in ("kicad-update","kicad-update:go","kicad-update:later")))
print(shown, apart, json.dumps({"row": row, "pane": pane}))' "$R")
read -r SHOWN APART DETAIL <<<"$U"
echo "INFO $DETAIL"
check "U1 the update row is drawn with Update and Not now" "$SHOWN"
check "U2 the update row and its buttons are outside the pane's rect (a row of its own above it)" "$APART" "$DETAIL"
# U3: Not now
AT=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]).get("kicad-update:later"); print("%d %d" % (r["x"] + r["w"] / 2, r["y"] + r["h"] / 2) if r else "")' "$R")
if [ -n "$AT" ]; then
  read -r AX AY <<<"$AT"
  # the client window's place on the screen (0,0 under i3; under mutter the window manager's): the container's root position less its own
  read -r OX OY < <(last_kicad_report "$OUT/client.log" ready | python3 -c 'import json,sys; c=json.loads(sys.stdin.read() or "{}").get("container") or {}; print(c.get("root_x",0)-c.get("x",0), c.get("root_y",0)-c.get("y",0))')
  python3 "$XINPUT_PY" click $((OX + AX)) $((OY + AY)); sleep 1.5
fi
GONE=$(python3 -c 'import json,sys; print(int("kicad-update" not in json.loads(sys.argv[1]).get("_ids",[])))' "$(rects)")
KEPT=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("kicad_update_later",""))' "$OUT/local-settings.json" 2>/dev/null || echo "")
check "U3 Not now puts the row away and keeps the version on this device" "$([ "$GONE" = 1 ] && [ "$KEPT" = "10.99.0-bp.13" ] && echo 1 || echo 0)" "row gone $GONE, kept \"$KEPT\""
kill "$CP" 2>/dev/null || true
finish_suite "embed_update_row_e2e ($EMBED_MODE)"
