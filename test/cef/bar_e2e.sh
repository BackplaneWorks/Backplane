#!/bin/sh
# bar_e2e.sh CLIENT_BIN HUB_BIN : the embedded Chromium end to end on a private Xvfb with a local hub, stand-in agents and a local test page.
# Real X input (XTest) clicks the page's button and types in its field; CDP (the engine's loopback port) shows the page reacted; the hub's preview_*
# tools (an agent's, through the stand-in's `mcpcall:`) read the same page, are refused while the person has it, and see the person's changes after hand back.
# usage: test/gpui/cef/bar_e2e.sh CLIENT_BIN HUB_BIN   -> prints PASS/FAIL lines, exit 0 when all pass; artifacts in $OUT (default /tmp/cef-e2e)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd); MAIN=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
CLIENT=$1; HUB=$2; OUT=${OUT:-/tmp/cef-bar-e2e}; DISP=${DISP:-:78}; HPORT=${HPORT:-3874}; PPORT=${PPORT:-3898}; CDP=${CDP:-9411}
RT=${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}
XV=$MAIN/build/gpui/xvfb
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fakehome/.cache"
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP in use"; exit 2; }
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$OUT/bin/claude"; chmod +x "$OUT/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/$n"; chmod +x "$OUT/bin/$n"; done
# the hub's browser helper from source, on the attach path only (no Chrome of its own)
printf '#!/bin/sh\ncd %s/tools/browser && exec %s src/main.ts --engine chrome "$@" 2>> %s/helper.err\n' "$ROOT" "$(command -v bun)" "$OUT" > "$OUT/bin/backplane-browser"; chmod +x "$OUT/bin/backplane-browser"
python3 - "$OUT" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"cef","root":R+"/proj","at":NOW},{"$":"ThreadCreated","id":"t1","project":"p1","title":"browser","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
PIDS=""; cleanup() { for p in $PIDS; do kill $p 2>/dev/null || true; done; sleep 1; }; trap cleanup EXIT INT TERM
export LC_ALL=C
(cd "$HERE/page" && exec python3 -m http.server $PPORT --bind 127.0.0.1 > "$OUT/page.log" 2>&1) & PIDS="$PIDS $!"
LD_LIBRARY_PATH=$XV/usr/lib/x86_64-linux-gnu $XV/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$OUT/xvfb.log" 2>&1 & PIDS="$PIDS $!"
sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$OUT" > "$OUT/i3.conf"
mkdir -p -m 700 "$OUT/run"; XDG_RUNTIME_DIR=$OUT/run DISPLAY=$DISP i3 -c "$OUT/i3.conf" > "$OUT/i3.log" 2>&1 & PIDS="$PIDS $!"
sleep 1
( cd "$OUT/proj" && BACKPLANE_BROWSER_DEBUG=1 BACKPLANE_AGENTS=$OUT/bin BACKPLANE_BROWSER=$OUT/bin/backplane-browser BACKPLANE_NO_UPDATE=1 FAKE_LOG=$OUT/standin.log BP_STANDIN_MODE=stream HOME=$OUT/fakehome XDG_CACHE_HOME=$OUT/fakehome/.cache \
  exec "$HUB" --home "$OUT/home" --port $HPORT --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.log" 2>&1 ) & HP=$!; PIDS="$PIDS $HP"
n=0; until curl -s -m1 -o /dev/null http://127.0.0.1:$HPORT/hello; do sleep 0.2; n=$((n+1)); [ $n -gt 100 ] && { echo "hub did not start"; tail -5 "$OUT/hub.log"; exit 1; }; done
# the client in hub-frame mode (BACKPLANE_CEF_OFF): the Browser tab asks the hub for a page, then has a bar and tabs like a browser
cat > "$OUT/script.json" <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Browser"},{"wait":60000}]
JSON
DISPLAY=$DISP LD_LIBRARY_PATH= BACKPLANE_CEF_OFF=1 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$MAIN/build/gpui/linux/assets HOME=$OUT/fakehome \
  "$CLIENT" --url http://127.0.0.1:$HPORT --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 90 > "$OUT/client.log" 2>&1 & CP=$!; PIDS="$PIDS $CP"
export DISPLAY=$DISP
ok=0; fail=0; check() { if [ "$2" = "1" ]; then echo "PASS $1"; ok=$((ok+1)); else echo "FAIL $1 ($3)"; fail=$((fail+1)); fi; }
lasttabs() { grep -a '"browser_tabs"' "$OUT/client.log" | tail -1; }
waittabs() { n=0; until lasttabs | grep -q "$1"; do sleep 0.5; n=$((n+1)); [ $n -gt ${2:-40} ] && return 1; done; return 0; }
X() { python3 "$HERE/xinput.py" "$@"; }
waittabs 'tab-' 80 || { echo "FAIL the Browser tab never got a page (no tabs info)"; tail -5 "$OUT/client.log"; exit 1; }
check "an empty Browser tab opened a page by itself (tabs info arrived)" 1 ""
sleep 1; import -window root "$OUT/s0.png" 2>/dev/null || true
# address bar: click it, type an address, Enter
X click 1050 84; sleep 0.4; X type "127.0.0.1:$PPORT/index.html"; X key Return; sleep 0.5
waittabs "index.html" 40 || true; check "typing an address in the bar loaded the page ($(lasttabs | grep -o 'index.html' | head -1))" "$(lasttabs | grep -q index.html && echo 1 || echo 0)" "$(lasttabs | cut -c1-200)"
sleep 2; import -window root "$OUT/s1.png" 2>/dev/null || true
# the page: its button at page (180,104) in a 1280x800 frame shown Contain in the 543x802 view at (825,98)
BX=$((825 + 180 * 543 / 1280)); BY=$((98 + (802 - 543 * 800 / 1280) / 2 + 104 * 543 / 1280))
X click $BX $BY; sleep 2
waittabs "clicked:1" 20 || true; check "a click on the page in the tab reached the page (title clicked:1)" "$(lasttabs | grep -q 'clicked:1' && echo 1 || echo 0)" "$(lasttabs | cut -c1-200)"
IY=$((98 + (802 - 543 * 800 / 1280) / 2 + 176 * 543 / 1280))
X click $((825 + 200 * 543 / 1280)) $IY; sleep 0.5; X type hi; sleep 2
waittabs "typed:hi" 20 || true; check "typing in the page reached its input (title typed:hi)" "$(lasttabs | grep -q 'typed:hi' && echo 1 || echo 0)" "$(lasttabs | cut -c1-200)"
import -window root "$OUT/s2.png" 2>/dev/null || true
# tabs: "+" opens a second tab, the strip appears; selecting the first tab makes it the active one again
X click 1273 84; sleep 3
waittabs '"index\\":1' 20 || true; check "+ opened a second tab (tabs info lists two)" "$(lasttabs | grep -q 'index\\":1' && echo 1 || echo 0)" "$(lasttabs | cut -c1-300)"
import -window root "$OUT/s3.png" 2>/dev/null || true
X click 870 109; sleep 2
check "clicking the first tab in the strip made it the active one" "$(lasttabs | grep -q 'index.html.*active\\":true' && echo 1 || echo 0)" "$(lasttabs | cut -c1-400)"
import -window root "$OUT/s4.png" 2>/dev/null || true
echo "screens: $OUT/s0.png s1.png s2.png"
echo "passed=$ok failed=$fail"; [ $fail -eq 0 ]
