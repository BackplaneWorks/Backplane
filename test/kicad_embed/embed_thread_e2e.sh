#!/bin/bash
# Thread away and back: the editor comes back as it was left (the same KiCad frame, on show and drawn) with no new Edit. Away and back twice
# the sidebar's way (the client's `select`), then twice through the command palette as a person does (XTest: the palette, the title, Return).
# fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_thread_e2e: fork only"; exit 0; }
export KEMBED_THREADS=2
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
# the sidebar lists the newest thread first: 0 is "Other thread 2" (t2), 1 is "Embedded KiCad E2E" (t1), where the editor is opened
s = [{"select": 1}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "edit"}}, {"wait": 3000}]
for i in range(2):
    s += [{"select": 0}, {"wait": 1500}, {"kicad": {"report": True, "tag": f"away{i}"}}, {"select": 1}, {"kicad": {"wait_painted": 5000, "kind": "PCB", "tag": f"back{i}"}}, {"wait": 2000}, {"kicad": {"report": True, "tag": f"later{i}"}}]
s += [{"mark": "palette"}, {"wait": 20000}, {"mark": "done"}, {"wait": 500}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 90 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 900); do grep -q '"name":"palette","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
E=$(last_kicad_report "$OUT/client.log" edit)
EDIT_XID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("shown") or 0)' "$E")
for i in 0 1; do
  A=$(last_kicad_report "$OUT/client.log" "away$i"); B=$(last_kicad_report "$OUT/client.log" "back$i"); L=$(last_kicad_report "$OUT/client.log" "later$i")
  check "away by the sidebar ($i): the other thread shows no editor" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(0 if r.get("pane") else 1)' "$A")"
  read -r SAME MS PAINT < <(python3 -c '
import json, sys
e, b, l = (json.loads(a or "{}") for a in sys.argv[1:4])
f = next((f for f in b.get("frames", []) if f.get("xid") == b.get("shown")), {})
print(int(bool(b.get("shown")) and b.get("shown") == e.get("shown") and l.get("shown") == e.get("shown") and (l.get("container") or {}).get("shown") is True), b.get("wait_ms", -1), f.get("painted", 0))' "$E" "$B" "$L")
  echo "PERF thread_back_sidebar$i ${MS}ms painted=$PAINT"
  check "back to the thread by the sidebar ($i): the same editor frame on show and drawn, and still 2 s later" "$([ "$SAME" = 1 ] && [ "${PAINT:-0}" -ge 15 ] && echo 1 || echo 0)" "same=$SAME painted=$PAINT"
done
eval "$(python3 "$DRIVER_PY" find-gpui --shell --client-log "$OUT/client.log")"; GXID=$GPUI_XID
# the editor's frame mapped and viewable on the screen (the X tree, not the client's word)
viewable() { python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys; x=int(sys.argv[1]); print(int(any(w["xid_int"]==x and w["shown"] for w in json.load(sys.stdin))))' "$EDIT_XID"; }
jump() { click_gpui_control "$OUT/client.log" "$GXID" "ui:Jump or run a command" >/dev/null 2>&1 || true; sleep 0.4; python3 "$XINPUT_PY" type "$1"; sleep 0.5; python3 "$XINPUT_PY" key Return; }
for i in 0 1; do
  jump "Other thread 2"; sleep 1.5
  AWAY=$(viewable)
  jump "Embedded KiCad E2E"; sleep 2.5
  BACK=$(viewable)
  shot "palette_back$i"
  check "away by the palette ($i): the editor leaves the screen" "$([ "$AWAY" = 0 ] && echo 1 || echo 0)" "viewable=$AWAY"
  check "back by the palette ($i): the same editor frame on screen" "$BACK" "viewable=$BACK"
done
for _ in $(seq 300); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done
wait "$CP" 2>/dev/null || true
finish_suite "embed_thread_e2e ($EMBED_MODE)"
