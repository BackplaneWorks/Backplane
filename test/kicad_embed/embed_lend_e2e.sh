#!/bin/bash
# A reserved key pressed in KiCad opens Backplane's palette (Ctrl+K); dismissed with nothing moved, the keyboard goes back to KiCad:
#   L1  Ctrl+K in KiCad opens the palette (Backplane has the keyboard)
#   L2  Escape shuts it and KiCad has the keyboard again: a plain hotkey (F2, zoom out) acts in the canvas with no click
#   L3  a second Escape goes to KiCad, not Backplane: the viewer stays open (before: it closed the viewer with the editor in it)
#   L4  Ctrl+K again and another thread chosen: the keyboard stays with Backplane (the editor is not on show there)
# XTest as a person types; the one click is the first, into the canvas. fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_lend_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
discover_bins; discover_kicad; init_isolated_dirs
KEMBED_THREADS=2 python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "ready"}}, {"mark": "ready"},
     {"wait": 60000}, {"mark": "done"}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 900); do grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" ready)
read -r SHOWN X Y W H < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("shown") or 0, f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
check "the board editor is shown" "$([ "$SHOWN" != 0 ] && echo 1 || echo 0)"
sleep 3
# the X focus is in KiCad's frame (or a window inside it)
in_kicad() { python3 -c 'import ctypes,ctypes.util,os,sys
x=ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype=ctypes.c_void_p; d=ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
w=ctypes.c_ulong(); r=ctypes.c_int(); x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(r)); f=w.value; want=int(sys.argv[1])
root=ctypes.c_ulong(); par=ctypes.c_ulong(); ch=ctypes.POINTER(ctypes.c_ulong)(); n=ctypes.c_uint()
for _ in range(30):
    if f == want: print(1); sys.exit()
    if not x.XQueryTree(d, ctypes.c_ulong(f), ctypes.byref(root), ctypes.byref(par), ctypes.byref(ch), ctypes.byref(n)) or par.value in (0, root.value): break
    f = par.value
print(0)' "$SHOWN"; }
focus_info() { python3 -c 'import ctypes,ctypes.util,os
x=ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype=ctypes.c_void_p; d=ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
w=ctypes.c_ulong(); r=ctypes.c_int(); x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(r)); f=w.value
root=ctypes.c_ulong(); par=ctypes.c_ulong(); ch=ctypes.POINTER(ctypes.c_ulong)(); n=ctypes.c_uint(); out=[]
for _ in range(8):
    nm=ctypes.c_char_p(); x.XFetchName(d, ctypes.c_ulong(f), ctypes.byref(nm)); out.append(f"{f:#x}:{(nm.value or b"").decode(errors="replace")[:30]}")
    if not x.XQueryTree(d, ctypes.c_ulong(f), ctypes.byref(root), ctypes.byref(par), ctypes.byref(ch), ctypes.byref(n)) or par.value in (0, root.value): break
    f = par.value
print(" < ".join(out))'; }
closed() { python3 -c 'import json,sys
n=0
for l in open(sys.argv[1], errors="ignore").read().splitlines()[int(sys.argv[2]):]:
    if "GPUI_METRIC " in l and "\"event\":\"shown\"" in l:
        try: m=json.loads(l.split("GPUI_METRIC ",1)[1])
        except Exception: continue
        if not m.get("open"): n+=1
print(n)' "$OUT/client.log" "$1"; }
# a hotkey's effect: KiCad's frame's own picture before and after (works under mutter's rootless Xwayland too)
hotkey_acts() {
  sleep 1.0
  import -window "$SHOWN" -crop "$((W / 2))x$((H / 2))+$((W / 4))+$((H / 4))" "$OUT/$1_a.png" 2>/dev/null || true
  python3 "$XINPUT_PY" key F2; sleep 1.2
  import -window "$SHOWN" -crop "$((W / 2))x$((H / 2))+$((W / 4))+$((H / 4))" "$OUT/$1_b.png" 2>/dev/null || true
  python3 "$XINPUT_PY" key Home; sleep 0.8
  python3 -c 'import sys
from PIL import Image, ImageChops
a, b = (Image.open(f).convert("RGB") for f in sys.argv[1:3])
print(ImageChops.difference(a, b).convert("L").point(lambda v: 255 if v > 24 else 0).histogram()[255])' "$OUT/$1_a.png" "$OUT/$1_b.png" 2>/dev/null || echo -1
}
# the keyboard to KiCad: one click in the canvas, low down (away from the middle's parts)
python3 "$XINPUT_PY" click $((X + W / 2)) $((Y + H - 120)); sleep 0.8
check "a click gives KiCad the keyboard" "$(in_kicad)"

# L1, L2, L3
N0=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" chord ctrl k; sleep 1.0
echo "INFO focus after Ctrl+K: $(focus_info)"
check "L1 Ctrl+K pressed in KiCad opens the palette" "$(tail -n +"$N0" "$OUT/client.log" | grep -q '"kicad_key_dispatch":"ctrl-k"' && [ "$(in_kicad)" = 0 ] && echo 1 || echo 0)" "$(tail -n +"$N0" "$OUT/client.log" | grep -o '"kicad_key[^}]*' | head -3 | tr '\n' ' ')"
python3 "$XINPUT_PY" key Escape; sleep 0.8
echo "INFO focus after Escape: $(focus_info)"
echo "INFO after Escape: $(tail -n +"$N0" "$OUT/client.log" | grep -o '"kicad_loan_back":[a-z]*,"moved":[a-z]*' | tail -1)"
check "L2 Escape shuts the palette and KiCad has the keyboard again" "$(in_kicad)"
C=$(hotkey_acts l2)
check "L2 then a plain hotkey (F2) acts in KiCad's canvas with no click" "$([ "$C" -gt 500 ] && echo 1 || echo 0)" "$C pixels changed"
N1=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" key Escape; sleep 1.2
check "L3 a second Escape goes to KiCad: the viewer stays open" "$([ "$(closed "$N1")" = 0 ] && [ "$(in_kicad)" = 1 ] && echo 1 || echo 0)" "viewer closed $(closed "$N1") times, keyboard in KiCad $(in_kicad)"

# L4: Ctrl+K, another thread: the keyboard stays with Backplane
N2=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" chord ctrl k; sleep 0.8
# the thread not on show (select 0 opens the newest, which may be either)
CUR=$(grep -o '"thread":"t[0-9]*","open":"board"' "$OUT/client.log" | tail -1 | sed 's/"thread":"\(t[0-9]*\)".*/\1/' || true)
if [ "$CUR" = t2 ]; then OTHER=t1; TITLE="Embedded KiCad E2E"; else OTHER=t2; TITLE="Other thread 2"; fi
echo "INFO on $CUR: going to $OTHER ($TITLE)"
python3 "$XINPUT_PY" type "$TITLE"; sleep 0.8
python3 "$XINPUT_PY" key Return; sleep 1.5
echo "INFO after choosing another thread: $(tail -n +"$N2" "$OUT/client.log" | grep -o '"kicad_loan_back":[a-z]*,"moved":[a-z]*' | tail -1)"
check "L4 Ctrl+K then another thread: the keyboard stays with Backplane" "$(tail -n +"$N2" "$OUT/client.log" | grep -q "\"thread\":\"$OTHER\"" && [ "$(in_kicad)" = 0 ] && echo 1 || echo 0)" "$(tail -n +"$N2" "$OUT/client.log" | grep -o '"event":"shown"[^}]*' | tail -1)"
kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_lend_e2e ($EMBED_MODE)"
