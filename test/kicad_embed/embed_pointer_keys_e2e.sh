#!/bin/bash
# Keys typed with Backplane holding the keyboard and the pointer resting over the embedded KiCad pane (no click): where do they go?
# Core X delivers a key to the pointer's window when it is inside the focus window, and KiCad's frame is reparented inside Backplane's
# top-level: the right-rail gate a2 saw the keys reach KiCad. CASE=b (default): the person's Edit (KiCad has the keys), then a click in
# the composer (the keyboard back to Backplane), then the pointer parked over the pane's canvas with one move, then keys typed.
# CASE=a: the automatic attach instead of Edit (no click in KiCad at all). Per step it logs the X focus window chain and the pointer's
# window chain; it types "ctl" with the pointer over the composer (the control) and "pan" with it over the pane, and reads the composer.
#   P1 control: with the pointer over the composer the keys land in it
#   P2 with the pointer over the pane they land in the composer too (else: which window got them, and whether KiCad opened something)
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
 {"wait":20000},{"read":"composer"},{"mark":"done"},{"wait":600000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" \
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
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 120 || true
TEXT=$(python3 -c 'import json,sys
t=None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l:
        try: o=json.loads(l.split("GPUI_METRIC ",1)[1])
        except Exception: continue
        if o.get("event")=="read" and o.get("what")=="composer": t=o.get("text")
print(json.dumps(t))' "$OUT/client.log")
echo "INFO composer: $TEXT; named windows on screen before/after typing over the pane: $W0 / $W1"
grep -o '"kicad_ev":"[^"]*"\|"kicad_keys_flip":{"to":[a-z]*' "$OUT/client.log" | tail -5 | sed 's/^/INFO client: /'
check "P1 control: with the pointer over the composer the keys land in it" "$(echo "$TEXT" | grep -q ctl && echo 1 || echo 0)" "$TEXT"
check "P2 with the pointer over the pane (no click) the keys land in the composer too" "$(echo "$TEXT" | grep -q "ctlpan" && echo 1 || echo 0)" "$TEXT; windows $W0 -> $W1"
finish_suite "embed_pointer_keys_e2e ($EMBED_MODE, case $CASE)"
