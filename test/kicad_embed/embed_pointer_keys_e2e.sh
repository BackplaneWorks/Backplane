#!/bin/bash
# Keys typed with Backplane holding the keyboard and the pointer resting over the embedded KiCad pane (no click): where do they go?
# Core X delivers a key to the pointer's window when it is inside the focus window, and KiCad's frame is reparented inside Backplane's
# top-level: the right-rail gate a2 saw the keys reach KiCad. CASE=b (default): the person's Edit (KiCad has the keys), then a click in
# the composer (the keyboard back to Backplane), then the pointer parked over the pane's canvas with one move, then keys typed.
# CASE=a: the automatic attach instead of Edit (no click in KiCad at all). Per step it logs the X focus window chain and the pointer's
# window chain; it types "ctl" with the pointer over the composer (the control) and "pan" with it over the pane, and reads the composer.
#   P1 control: with the pointer over the composer the keys land in it
#   P2 with the pointer over the pane they land in the composer too (else: which window got them, and whether KiCad opened something)
#   P3 a click in the pane (KiCad's keys), a click in the composer and at once a key over the pane, RACE (20) times: all in the composer
#   P4 another X window focused and back: Backplane inactive then active, keys over the pane in the composer
#   P6 a dead key (us intl) and a letter over the pane compose in the composer   P5 the wheel over the pane still zooms KiCad (no click)
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_pointer_keys_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
CASE=${CASE:-b}
if [ "$CASE" = a ]; then OPEN='{"select":0},{"press":"rail:Board"},{"kicad":{"attach":"board","why":"auto"}}'
else OPEN='{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"}'; fi
cat >"$OUT/script.json" <<JSON
[$OPEN,{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":3000},{"kicad":{"report":true,"tag":"ready"}},{"mark":"ready"},
 {"wait":${COMPOSER_READ_MS:-90000}},{"read":"composer"},{"mark":"done"},{"wait":600000}]
JSON
# a UTF-8 locale for the client, as a person has (its compose table: P6)
LC_ALL=C.UTF-8 HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" \
  --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 600 || { echo "FAIL KiCad never came up"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
read -r TOP PX PY PW PH CONT < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); p=r.get("pane") or {}; c=r.get("container") or {}
print("0x%x" % int(r.get("toplevel") or 0), p.get("x",0), p.get("y",0), p.get("w",0), p.get("h",0), "0x%x" % int(c.get("xid") or 0))' "$R")
read -r TX TY < <(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if w["xid_int"]==int(sys.argv[1],0)]; print(w[0]["root_x"], w[0]["root_y"]) if w else print(0, 0)' "$TOP")
# the composer's middle, from the client's rects (window coordinates), in root coordinates
read -r CX CY < <(python3 - "$OUT/client.log" "$TX" "$TY" <<'PY'
import json, sys
c = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"rects":{' in l and '"composer"' in l:
        try: c = json.loads(l.split("GPUI_METRIC ", 1)[1])["rects"]["composer"]
        except Exception: pass
print(int(sys.argv[2]) + int(c["x"] + c["w"] / 2), int(sys.argv[3]) + int(c["y"] + c["h"] / 3)) if c else print(0, 0)
PY
)
PAX=$((TX + PX + PW / 2)); PAY=$((TY + PY + PH / 2))
pointer_chain() { python3 -c 'import ctypes,ctypes.util,os
x=ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype=ctypes.c_void_p; d=ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
U=ctypes.c_ulong; I=ctypes.c_int; root=x.XDefaultRootWindow; root.restype=U; root.argtypes=[ctypes.c_void_p]; w=root(d); out=[]
for _ in range(12):
    r=U(); c=U(); rx=I(); ry=I(); wx=I(); wy=I(); m=ctypes.c_uint()
    x.XQueryPointer(d, U(w), ctypes.byref(r), ctypes.byref(c), ctypes.byref(rx), ctypes.byref(ry), ctypes.byref(wx), ctypes.byref(wy), ctypes.byref(m))
    if not c.value: break
    w=c.value; out.append(hex(w))
print(" > ".join(out))' 2>/dev/null; }
say() { echo "INFO $1: focus [$(xfocus_chain)] pointer [$(pointer_chain)]"; }
type_word() { local w=$1 i; for ((i = 0; i < ${#w}; i++)); do python3 "$XINPUT_PY" key "${w:i:1}" >/dev/null; sleep 0.15; done; }
kicad_wins() { xwininfo -root -tree 2>/dev/null | grep -cE '"[^"]+"' || true; }

echo "INFO case $CASE; top $TOP container $CONT; pane root ($PAX, $PAY); composer root ($CX, $CY)"
say "after open"
# the keyboard back to Backplane: a click in the composer
python3 "$XINPUT_PY" click "$CX" "$CY" >/dev/null; sleep 1.2
say "after a click in the composer"
type_word ctl; sleep 0.5
# the pointer parked over the pane's canvas, one move, no click
python3 "$XINPUT_PY" move "$PAX" "$PAY" >/dev/null; sleep 1.0
say "pointer over the pane"
W0=$(kicad_wins)
type_word pan; sleep 1.0
say "after typing over the pane"
W1=$(kicad_wins)

# P3 (the race a click opens): KiCad given the keys by a click in the pane, then a click in the composer and at once the pointer back over
# the pane and a key: RACE times, each "r" must reach the composer
RACE=${RACE:-20}
for k in $(seq "$RACE"); do
  python3 "$XINPUT_PY" click "$PAX" "$PAY" >/dev/null; sleep 0.6
  python3 "$XINPUT_PY" click "$CX" "$CY" >/dev/null; python3 "$XINPUT_PY" move "$PAX" "$PAY" >/dev/null; python3 "$XINPUT_PY" key r >/dev/null
  sleep 0.3
done
say "after the race"
W2=$(kicad_wins)
# P4: another X window takes the focus and gives it back (Backplane inactive, then active again), then keys with the pointer over the pane.
# A zenity entry (it takes the keyboard), floating where it asked, given the focus as a pager would (_NET_ACTIVE_WINDOW)
A0=$(grep -c '"window_active":' "$OUT/client.log" || true)
zenity --entry --title "other window" --text other >/dev/null 2>&1 & XM=$!
OW=""; for _ in $(seq 40); do OW=$(xwininfo -root -tree 2>/dev/null | grep -m1 '"other window"' | awk '{print $1}'); [ -n "$OW" ] && break; sleep 0.25; done
[ "${KEMBED_WM:-i3}" = mutter ] || i3-msg -s "$OUT/i3.sock" '[title="other window"] floating enable, move position 20 20' >/dev/null 2>&1; sleep 0.5
[ -n "$OW" ] && python3 "$KEMBED_LIB/wm_ctl.py" activate "$OW" >/dev/null 2>&1; sleep 1.0
say "another window focused"
FOC_OTHER=$(xfocus_chain | grep -q "other window\|zenity" && echo 1 || echo 0)
INACTIVE=$(tail -n +"$((A0 + 1))" <(grep '"window_active":' "$OUT/client.log") | grep -q '"window_active":false' && echo 1 || echo 0)
A1=$(grep -c '"window_active":' "$OUT/client.log" || true)
python3 "$XINPUT_PY" click "$CX" "$CY" >/dev/null; sleep 0.8
ACTIVE=$(tail -n +"$((A1 + 1))" <(grep '"window_active":' "$OUT/client.log") | tail -1 | grep -q '"window_active":true' && echo 1 || echo 0)
python3 "$XINPUT_PY" move "$PAX" "$PAY" >/dev/null; sleep 0.5
type_word back; sleep 0.5
say "back, typed over the pane"
kill "$XM" 2>/dev/null
# P6: a dead key over the pane (us intl: ' is dead_acute, then e): the composed letter reaches the composer, nothing of it KiCad
if [ "${DEADKEY:-1}" = 1 ] && command -v setxkbmap >/dev/null; then
  setxkbmap -layout us -variant intl 2>/dev/null; sleep 1.0
  python3 "$XINPUT_PY" key dead_acute >/dev/null; sleep 0.2; python3 "$XINPUT_PY" key e >/dev/null; sleep 0.5
  setxkbmap -layout us 2>/dev/null; sleep 0.5
fi
# P5: the wheel over the pane still reaches KiCad's canvas with Backplane holding the keys (no click): the pane's picture changes
shot wheel-before; for _ in 1 2 3; do python3 "$XINPUT_PY" wheel "$PAX" "$PAY" 1 >/dev/null; sleep 0.2; done; sleep 1.5; shot wheel-after
WHEEL=$(python3 - "$OUT/shots/wheel-before.png" "$OUT/shots/wheel-after.png" "$PAX" "$PAY" <<'PY'
import sys
from PIL import Image, ImageChops
a, b = Image.open(sys.argv[1]).convert("L"), Image.open(sys.argv[2]).convert("L"); x, y = int(sys.argv[3]), int(sys.argv[4])
box = (x - 150, y - 150, x + 150, y + 150)
d = ImageChops.difference(a.crop(box), b.crop(box)); print(round(sum(1 for v in d.getdata() if v > 24) / (300 * 300), 3))
PY
)
say "after the wheel"
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 600 || echo "INFO the client never read the composer"
TEXT=$(python3 -c 'import json,sys
t=None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l:
        try: o=json.loads(l.split("GPUI_METRIC ",1)[1])
        except Exception: continue
        if o.get("event")=="read" and o.get("what")=="composer": t=o.get("text")
print(json.dumps(t))' "$OUT/client.log")
echo "INFO composer: $TEXT; named windows on screen before/after typing over the pane: $W0 / $W1, after the race: $W2; wheel changed ${WHEEL:-?} of the pane's middle"
grep -o '"kicad_ev":"[^"]*"\|"kicad_keys_flip":{"to":[a-z]*' "$OUT/client.log" | tail -5 | sed 's/^/INFO client: /'
check "P1 control: with the pointer over the composer the keys land in it" "$(echo "$TEXT" | grep -q ctl && echo 1 || echo 0)" "$TEXT"
check "P2 with the pointer over the pane (no click) the keys land in the composer too" "$(echo "$TEXT" | grep -q "ctlpan" && echo 1 || echo 0)" "$TEXT; windows $W0 -> $W1"
RS=$(python3 -c 'import json,sys; t=json.loads(sys.argv[1]) or ""; print(t.count("r"))' "$TEXT")
check "P3 the race: a click back into Backplane and a key at once over the pane, $RACE times: every key in the composer" "$([ "$RS" -ge "$RACE" ] && echo 1 || echo 0)" "$RS of $RACE"
check "P4 another window focused and back: keys over the pane land in the composer" "$(echo "$TEXT" | grep -q "back" && echo 1 || echo 0)" "$TEXT"
check "P4 (precondition) the other window had the keyboard" "$FOC_OTHER"
check "P4 Backplane went inactive while the other window had the keyboard" "$INACTIVE"
check "P4 and active again after the click back (the keyboard on the proxy)" "$ACTIVE"
[ "${DEADKEY:-1}" = 1 ] && check "P6 a dead key and a letter typed over the pane compose in the composer (é)" "$(python3 -c 'import json,sys; print(1 if "\u00e9" in (json.loads(sys.argv[1]) or "") else 0)' "$TEXT")" "$TEXT"
check "P5 the wheel over the pane still reaches KiCad (its picture changes, no click)" "$(python3 -c 'import sys; print(1 if float(sys.argv[1] or 0) > 0.05 else 0)' "${WHEEL:-0}")" "${WHEEL:-?}"
finish_suite "embed_pointer_keys_e2e ($EMBED_MODE, case $CASE)"
