#!/bin/bash
# The Browser tab's Chromium (a native child window of the client's top-level, like KiCad's frame) and keys typed while Backplane holds the
# keyboard with the pointer resting over the page: the same core X rule (a key goes to the window under the pointer inside the focus window).
# A measurement for the issues lead's lane (the Browser), next to embed_pointer_keys_e2e (the KiCad pane's focus proxy). Needs a client built
# with the embedded Chromium (feature cef: a dev-build stage). WITH_PANE=1: a KiCad pane opened (Edit) first, so the client's focus proxy is
# in use in that window; WITH_PANE=0 (default): no pane, the window as the Browser alone has it.
#   B1 control: keys typed with the pointer over the composer land in it
#   B2 with the pointer over the page (no click) the keys land in the composer (else the page's keydown log has them)
#   B3 a click in the page gives the page the keyboard (the Browser's own hand-in; the page logs them)
#   B4 a click back in the composer, the pointer over the page again: the keys land in the composer
# The page logs every keydown into its title (`k:<keys>`), read from the client's cef report.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
WITH_PANE=${WITH_PANE:-0}
[ "$WITH_PANE" = 1 ] && discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
PAGE='data:text/html,<body style="margin:0;background:%23e8f0ff"><h1>keys</h1><input id=t><script>let k="";document.addEventListener("keydown",e=>{k+=e.key;document.title="k:"+k})</script></body>'
if [ "$WITH_PANE" = 1 ]; then
  OPEN='{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":3000},'
else OPEN='{"select":0},{"wait":2000},'; fi
cat >"$OUT/script.json" <<JSON
[$OPEN{"click":"rail:Browser"},{"wait":5000},{"cef":{"goto":"$PAGE"}},{"wait":4000},{"cef":{"report":true}},{"mark":"ready"},
 {"wait":${READ_MS:-40000}},{"read":"composer"},{"cef":{"report":true}},{"mark":"done"},{"wait":600000}]
JSON
HOME="$OUT/fh" LC_ALL=C.UTF-8 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_CEF_PROFILE="$OUT/prof" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" \
  --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 400 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 800 || { echo "FAIL the Browser never came up"; exit 1; }
TOP=$(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if "Backplane GPUI" in (w.get("title") or "")]; print(w[0]["xid"] if w else "")')
read -r TX TY < <(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if w["xid"]==sys.argv[1]]; print(w[0]["root_x"], w[0]["root_y"]) if w else print(0, 0)' "$TOP")
# the page's middle (the last cef report's view origin and frame) and the composer's middle (the client's rects), in root coordinates
read -r GX GY CX CY < <(python3 - "$OUT/client.log" "$TX" "$TY" <<'PY'
import json, sys
r = c = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " not in l: continue
    try: o = json.loads(l.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    if isinstance(o.get("rects"), dict) and "composer" in o["rects"]: c = o["rects"]["composer"]
    if o.get("event") == "cef" and o.get("view_x") is not None: r = o
tx, ty = int(sys.argv[2]), int(sys.argv[3])
gx = tx + int(r["view_x"] + r["frame"][0] / 2) if r else 0; gy = ty + int(r["view_y"] + r["frame"][1] / 2) if r else 0
print(gx, gy, tx + int(c["x"] + c["w"] / 2) if c else 0, ty + int(c["y"] + c["h"] / 3) if c else 0)
PY
)
echo "INFO with pane $WITH_PANE; top $TOP; page root ($GX, $GY); composer root ($CX, $CY)"
if [ "${GX:-0}" -le 0 ] || [ "${CX:-0}" -le 0 ]; then check "B0 the page's and the composer's places are known" 0 "page ($GX, $GY) composer ($CX, $CY)"; finish_suite "embed_pointer_keys_browser_e2e"; exit 1; fi
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
python3 "$XINPUT_PY" click "$CX" "$CY" >/dev/null; sleep 1.0; say "a click in the composer"
type_word ctl; sleep 0.4
python3 "$XINPUT_PY" move "$GX" "$GY" >/dev/null; sleep 0.8; say "pointer over the page"
type_word pg; sleep 0.8
python3 "$XINPUT_PY" click "$GX" "$GY" >/dev/null; sleep 1.0; say "a click in the page"
type_word in; sleep 0.8
python3 "$XINPUT_PY" click "$CX" "$CY" >/dev/null; sleep 1.0
python3 "$XINPUT_PY" move "$GX" "$GY" >/dev/null; sleep 0.8; say "back in the composer, pointer over the page"
type_word bk; sleep 0.8
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 600 || echo "INFO the client never read the composer"
read -r TEXT TITLE < <(python3 - "$OUT/client.log" <<'PY'
import json, sys
t = title = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " not in l: continue
    try: o = json.loads(l.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    if o.get("event") == "read" and o.get("what") == "composer": t = o.get("text")
    if o.get("event") == "cef" and "title" in o: title = o.get("title")
print(json.dumps(t), json.dumps(title))
PY
)
echo "INFO composer: $TEXT; the page's keydown log (title): $TITLE"
grep -o '"window_active":[a-z]*' "$OUT/client.log" | uniq -c | tr '\n' ' ' | sed 's/^/INFO window_active changes: /'; echo
has() { python3 -c 'import json,sys; print(1 if sys.argv[2] in (json.loads(sys.argv[1]) or "") else 0)' "$1" "$2"; }
check "B1 control: keys over the composer land in it" "$(has "$TEXT" ctl)" "$TEXT"
check "B2 with the pointer over the page (no click) the keys land in the composer" "$(has "$TEXT" ctlpg)" "composer $TEXT, page $TITLE"
check "B3 a click in the page gives it the keyboard (the page logs them)" "$(has "$TITLE" in)" "page $TITLE"
check "B4 back in the composer, the pointer over the page: keys in the composer" "$(has "$TEXT" bk)" "composer $TEXT, page $TITLE"
finish_suite "embed_pointer_keys_browser_e2e (with pane $WITH_PANE)"
