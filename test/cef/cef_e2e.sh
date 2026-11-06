#!/bin/sh
# cef_e2e.sh CLIENT_BIN HUB_BIN : the embedded Chromium end to end on a private Xvfb with a local hub, stand-in agents and a local test page.
# Real X input (XTest) clicks the page's button and types in its field; CDP (the engine's loopback port) shows the page reacted; the hub's preview_*
# tools (an agent's, through the stand-in's `mcpcall:`) read the same page, are refused while the person has it, and see the person's changes after hand back.
# usage: test/cef/cef_e2e.sh CLIENT_BIN HUB_BIN   -> prints PASS/FAIL lines, exit 0 when all pass; artifacts in $OUT (default /tmp/cef-e2e)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); MAIN=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
CLIENT=$1; HUB=$2; OUT=${OUT:-/tmp/cef-e2e}; DISP=${DISP:-:77}; HPORT=${HPORT:-3873}; PPORT=${PPORT:-3898}; CDP=${CDP:-9411}
RT=${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}
XV=$MAIN/build/gpui/xvfb; POLICY=${GPUI_POLICY:-$MAIN/build/gpui/slim/bp-policy-page}; WINBIN=${GPUI_WINDOW_BIN:-$MAIN/build/gpui/hub/new/window}
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fakehome/.cache"
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP in use"; exit 2; }
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$OUT/bin/claude"; chmod +x "$OUT/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/$n"; chmod +x "$OUT/bin/$n"; done
# the hub's browser helper from source, on the attach path only (no Chrome of its own)
printf '#!/bin/sh\ncd %s/tools/browser && exec %s src/main.ts --engine chrome "$@"\n' "$ROOT" "$(command -v bun)" > "$OUT/bin/backplane-browser"; chmod +x "$OUT/bin/backplane-browser"
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
( cd "$OUT/proj" && BACKPLANE_AGENTS=$OUT/bin BACKPLANE_BROWSER=$OUT/bin/backplane-browser BACKPLANE_NO_UPDATE=1 FAKE_LOG=$OUT/standin.log BP_STANDIN_MODE=stream HOME=$OUT/fakehome XDG_CACHE_HOME=$OUT/fakehome/.cache \
  exec "$HUB" --home "$OUT/home" --port $HPORT --host 127.0.0.1 --no-tailscale --foreground --policy-bin "$POLICY" --window-bin "$WINBIN" > "$OUT/hub.log" 2>&1 ) & HP=$!; PIDS="$PIDS $HP"
n=0; until curl -s -m1 -o /dev/null http://127.0.0.1:$HPORT/hello; do sleep 0.2; n=$((n+1)); [ $n -gt 100 ] && { echo "hub did not start"; tail -5 "$OUT/hub.log"; exit 1; }; done
EV='mcpcall:{"name":"preview_evaluate","arguments":{"expression":"document.title"}}'
cat > "$OUT/script.json" <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Browser"},{"wait":2500},{"cef":{"goto":"http://127.0.0.1:$PPORT/index.html"}},{"wait":4500},{"cef":{"report":true}},
 {"type":"$(printf '%s' "$EV" | sed 's/"/\\"/g')"},{"send":true},{"wait":9000},
 {"mark":"human"},{"wait":18000},{"cef":{"report":true}},
 {"type":"$(printf '%s' "$EV" | sed 's/"/\\"/g')"},{"send":true},{"wait":9000},
 {"cef":{"handback":true}},{"wait":1500},{"type":"$(printf '%s' "$EV" | sed 's/"/\\"/g')"},{"send":true},{"wait":9000},{"cef":{"report":true}},{"wait":500}]
JSON
DISPLAY=$DISP LD_LIBRARY_PATH=$RT BACKPLANE_CEF=$RT BACKPLANE_CEF_CDP_PORT=$CDP BACKPLANE_CEF_NO_GPU=1 BACKPLANE_CEF_PROFILE=$OUT/profile VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 \
  BACKPLANE_GPUI_ASSETS=$MAIN/build/gpui/linux/assets HOME=$OUT/fakehome \
  "$CLIENT" --url http://127.0.0.1:$HPORT --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 > "$OUT/client.log" 2>&1 & CP=$!; PIDS="$PIDS $CP"
export DISPLAY=$DISP
ok=0; fail=0; check() { if [ "$2" = "1" ]; then echo "PASS $1"; ok=$((ok+1)); else echo "FAIL $1 ($3)"; fail=$((fail+1)); fi; }
title() { curl -s -m2 http://127.0.0.1:$CDP/json/list | python3 -c "import json,sys;[print(t['title']) for t in json.load(sys.stdin) if t.get('type')=='page' and '127.0.0.1:$PPORT' in t.get('url','')]" 2>/dev/null | head -1; }
waitfor() { n=0; until grep -q "$1" "$OUT/client.log" 2>/dev/null; do sleep 0.3; n=$((n+1)); [ $n -gt ${2:-200} ] && return 1; done; return 0; }
waitfor '"non_white_samples"' 200 || { echo "FAIL no cef report from the client"; tail -20 "$OUT/client.log"; exit 1; }
sleep 0.5
REP=$(grep '"event":"cef"' "$OUT/client.log" | grep '"non_white_samples"' | head -1)
echo "report1: $REP"
python3 - "$REP" <<'PY' > "$OUT/r1.txt"
import json,sys; r=json.loads(sys.argv[1][sys.argv[1].index('{'):]); print(int(r.get('view_x',0)), int(r.get('view_y',0)), r.get('non_white_samples',0), r.get('url',''))
PY
read VX VY LIT URL < "$OUT/r1.txt"
check "page loaded and painted into the gpui image (non-white samples=$LIT)" "$([ "$LIT" -gt 20 ] && echo 1 || echo 0)" "$LIT"
check "client reports the test page url" "$(case "$URL" in *index.html) echo 1;; *) echo 0;; esac)" "$URL"
# window position on the screen (no window manager: the window sits at its own origin)
WX=$(xwininfo -root -tree | grep -i "Backplane GPUI" | head -1 | sed -E 's/.*[0-9]+x[0-9]+\+(-?[0-9]+)\+(-?[0-9]+).*/\1/' || echo 0)
WY=$(xwininfo -root -tree | grep -i "Backplane GPUI" | head -1 | sed -E 's/.*[0-9]+x[0-9]+\+(-?[0-9]+)\+(-?[0-9]+).*/\2/' || echo 0)
echo "window at $WX,$WY view origin $VX,$VY"
# agent first (page belongs to the agent): its read of document.title
waitfor '"event":"mark"' 400 || { echo "FAIL no human mark"; exit 1; }
sleep 1
A1=$(grep 'MCPCALL' "$OUT/standin.log" | sed -n 1p | cut -c1-300 || true)
check "agent (preview_evaluate via hub) read the page the user sees before any click" "$(echo "$A1" | grep -q 'cef-test-start' && echo 1 || echo 0)" "$A1"
T0=$(title); check "CDP sees the initial title" "$([ "$T0" = cef-test-start ] && echo 1 || echo 0)" "$T0"
# the person clicks the button with real X events: page-local (100+80, 80+24)
BX=$((WX+VX+180)); BY=$((WY+VY+104))
python3 "$HERE/xinput.py" click $BX $BY; sleep 1
T1=$(title); check "real X click on the page button reached the page (title '$T1')" "$([ "$T1" = clicked:1 ] && echo 1 || echo 0)" "$T1"
python3 "$HERE/xinput.py" click $((WX+VX+200)) $((WY+VY+176)); sleep 0.5
python3 "$HERE/xinput.py" type hello; sleep 1
T2=$(title); check "real X typing reached the page input (title '$T2')" "$([ "$T2" = typed:hello ] && echo 1 || echo 0)" "$T2"
python3 "$HERE/xinput.py" wheel $((WX+VX+300)) $((WY+VY+300)) -1; sleep 0.5
import -window root "$OUT/screen-after-input.png" 2>/dev/null || true
wait $CP || true
A2=$(grep 'MCPCALL' "$OUT/standin.log" | sed -n 2p | cut -c1-300 || true)
A3=$(grep 'MCPCALL' "$OUT/standin.log" | sed -n 3p | cut -c1-300 || true)
check "agent refused while the person drives" "$(echo "$A2" | grep -qi 'person has the browser' && echo 1 || echo 0)" "$A2"
check "after hand back the agent sees the person's input (typed:hello)" "$(echo "$A3" | grep -q 'typed:hello' && echo 1 || echo 0)" "$A3"
echo "passed=$ok failed=$fail; artifacts $OUT"; [ $fail -eq 0 ]
