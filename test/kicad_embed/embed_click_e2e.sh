#!/bin/bash
# One click into the embedded editor gives it the keyboard: a reserved chord (Ctrl+K) pressed right after reaches Backplane, and a key KiCad
# owns reaches KiCad. XTest clicks as a person does (press, ~60 ms, release); never a second click. fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_click_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "ready"}}, {"mark": "ready"},
     {"wait": 9000}, {"kicad": {"report": True, "tag": "after"}}, {"mark": "done"}, {"wait": 2000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 60 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 900); do grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" ready)
read -r SHOWN X Y W H < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("shown") or 0, f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
check "the board editor is shown" "$([ "$SHOWN" != 0 ] && echo 1 || echo 0)"
# one click on the canvas, as a person gives KiCad the keyboard: KEMBED_CLICK_AFTER seconds after the editor is drawn (default 3: the
# other editors' pre-warm runs 2 s after the first is placed, and must not take the keyboard back)
sleep "${KEMBED_CLICK_AFTER:-3}"
N0=$(grep -c '"kicad_ev":"key ctrl-k"' "$OUT/client.log" || true)
xfocus() { python3 -c 'import ctypes,ctypes.util,os
x=ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype=ctypes.c_void_p; d=ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
w=ctypes.c_ulong(); r=ctypes.c_int(); x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(r)); print(w.value)'; }
python3 "$XINPUT_PY" click $((X + W / 2)) $((Y + H / 2)); sleep 0.4
echo "INFO x focus after the click: $(xfocus) (editor $SHOWN)"
python3 "$XINPUT_PY" chord ctrl k; sleep 0.8
echo "INFO x focus after Ctrl+K: $(xfocus)"
N1=$(grep -c '"kicad_ev":"key ctrl-k"' "$OUT/client.log" || true)
check "one click, then Ctrl+K in KiCad reaches Backplane" "$([ "$N1" -gt "$N0" ] && echo 1 || echo 0)" "kicad_ev: $(grep -o '"kicad_ev":"[^"]*"' "$OUT/client.log" | tr '\n' ' ')"
for _ in $(seq 150); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done
wait "$CP" 2>/dev/null || true
finish_suite "embed_click_e2e ($EMBED_MODE)"
