#!/bin/sh
# cef_e2e.sh CLIENT_BIN HUB_BIN : the embedded Chromium end to end on a private Xvfb with a local hub, stand-in agents and a local test page.
# Real X input (XTest) clicks the page's button and types in its field; CDP (the engine's loopback port) shows the page reacted; the hub's preview_*
# tools (an agent's, through the stand-in's `mcpcall:`) read the same page, are refused while the person has it, and see the person's changes after hand back.
# usage: test/cef/cef_e2e.sh CLIENT_BIN HUB_BIN   -> prints PASS/FAIL lines, exit 0 when all pass; artifacts in $OUT (default /tmp/cef-e2e)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); MAIN=${GPUI_MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}
fp() { python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'; }
CLIENT=$1; HUB=$2; OUT=${OUT:-/tmp/cef-e2e}; DISP=${DISP:-:77}; HPORT=${HPORT:-$(fp)}; PPORT=${PPORT:-$(fp)}; CDP=${CDP:-$(fp)}
RT=${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}
XV=${XVFB_ROOT:-}; for d in $XV $MAIN/build/gpui/xvfb $HOME/.cache/xvfb-root $HOME/.local/xvfb-root; do [ -x "$d/usr/bin/Xvfb" ] && { XV=$d; break; }; done   # an unpacked Xvfb (no Xvfb package on the desktop)
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fakehome/.cache"
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP in use"; exit 2; }
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$OUT/bin/claude"; chmod +x "$OUT/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/$n"; chmod +x "$OUT/bin/$n"; done
# the hub's browser helper from source, on the attach path only (no Chrome of its own)
if [ -n "${E2E_BROWSER:-}" ]; then printf '#!/bin/sh\nexec %s "$@" 2>> %s/helper.err\n' "$E2E_BROWSER" "$OUT" > "$OUT/bin/backplane-browser"   # a release archive's own helper
else printf '#!/bin/sh\ncd %s/tools/browser && exec %s src/main.ts --engine chrome "$@" 2>> %s/helper.err\n' "$ROOT" "$(command -v bun)" "$OUT" > "$OUT/bin/backplane-browser"; chmod +x "$OUT/bin/backplane-browser"; fi
chmod +x "$OUT/bin/backplane-browser"
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
EV='mcpcall:{"name":"preview_evaluate","arguments":{"expression":"document.title"}}'
cat > "$OUT/script.json" <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Browser"},{"wait":2500},{"cef":{"goto":"http://127.0.0.1:$PPORT/index.html"}},{"wait":4500},{"cef":{"report":true}},
 {"type":"$(printf '%s' "$EV" | sed 's/"/\\"/g')"},{"send":true},{"wait":9000},
 {"mark":"human"},{"wait":18000},{"cef":{"report":true}},
 {"type":"$(printf '%s' "$EV" | sed 's/"/\\"/g')"},{"send":true},{"wait":9000},
 {"cef":{"handback":true}},{"wait":1500},{"type":"$(printf '%s' "$EV" | sed 's/"/\\"/g')"},{"send":true},{"wait":9000},{"cef":{"report":true}},
 {"mark":"remote"},{"wait":50000},{"cef":{"report":true}},
 {"cef":{"tab":0}},{"cef":{"goto":"http://127.0.0.1:$PPORT/bench.html"}},{"wait":4000},{"cef":{"report":true}},{"wait":3000},{"cef":{"report":true}},{"cef":{"latency":true}},{"wait":1500},{"cef":{"report":true}},{"wait":500}]
JSON
DISPLAY=$DISP LD_LIBRARY_PATH=$RT BACKPLANE_CEF=$RT BACKPLANE_CEF_CDP_PORT=$CDP BACKPLANE_CEF_NO_GPU=1 BACKPLANE_CEF_TEST_FILE=$HERE/page/test-upload.txt BACKPLANE_CEF_PROFILE=$OUT/profile VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 \
  BACKPLANE_GPUI_ASSETS=${E2E_ASSETS:-$MAIN/build/gpui/linux/assets} HOME=$OUT/fakehome \
  "$CLIENT" --url http://127.0.0.1:$HPORT --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 260 > "$OUT/client.log" 2>&1 & CP=$!; PIDS="$PIDS $CP"
export DISPLAY=$DISP
ok=0; fail=0; check() { if [ "$2" = "1" ]; then echo "PASS $1"; ok=$((ok+1)); else echo "FAIL $1 ($3)"; fail=$((fail+1)); fi; }
titles() { curl -s -m2 http://127.0.0.1:$CDP/json/list | python3 -c "import json,sys;[print(t.get('type'),t.get('title'),t.get('url')) for t in json.load(sys.stdin)]" 2>/dev/null; }
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
A1=$(grep 'MCPCALL' "$OUT/standin.log" | uniq | sed -n 1p | cut -c1-300 || true)
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

# ---- remote mirroring, downloads, file dialog, teach recording, popups with opener ----
waitfor '"name":"remote"' 400 || { echo "FAIL no remote mark"; }
sleep 1
HTOK="$OUT/home/token"; PORTH=$HPORT
# no watcher, no frames: with only the desktop client attached (it draws the CEF page itself) the helper has cast nothing and written no JPEG
check "no frames are made while only the desktop client watches (no shots/t1.jpg, no screencast)" "$([ ! -e "$OUT/home/shots/t1.jpg" ] && ! grep -q 'screencast on' "$OUT/helper.err" && echo 1 || echo 0)" "$(ls "$OUT/home/shots" 2>&1 | head -2)"
# a phone/web client: its screen.jpg request is the lease; the screencast turns on and the JPEG is the live page
curl -s -m6 -H "Authorization: Bearer $(cat $HTOK)" http://127.0.0.1:$HPORT/bots/t1/screen.jpg -o "$OUT/screen.jpg" || true
SZ=$(wc -c < "$OUT/screen.jpg" 2>/dev/null || echo 0); MAGIC=$(head -c 3 "$OUT/screen.jpg" | od -An -tx1 | tr -d ' ')
check "a lease (screen.jpg request) makes the hub serve the live CEF page as a JPEG ($SZ bytes)" "$([ "$MAGIC" = ffd8ff ] && [ "$SZ" -gt 3000 ] && echo 1 || echo 0)" "$MAGIC $SZ"
for i in 1 2 3 4 5 6; do curl -s -m3 -H "Authorization: Bearer $(cat $HTOK)" http://127.0.0.1:$HPORT/bots/t1/screen.jpg -o /dev/null; sleep 0.5; done
check "the screencast is on while it is leased" "$(grep -q 'screencast on t1' "$OUT/helper.err" && echo 1 || echo 0)" ""
sleep 5; M1=$(stat -c %Y "$OUT/home/shots/t1.jpg" 2>/dev/null || echo 0); N1=$(stat -c %s "$OUT/home/shots/t1.jpg" 2>/dev/null); sleep 3; M2=$(stat -c %Y "$OUT/home/shots/t1.jpg" 2>/dev/null || echo 0)
check "released: the screencast is off again and nothing is written" "$(grep -q 'screencast off t1' "$OUT/helper.err" && [ "$M1" = "$M2" ] && echo 1 || echo 0)" "$M1 $M2"
python3 "$HERE/xinput.py" move $((WX+VX+50)) $((WY+VY+50)); sleep 0.3
# a phone: take over, tap the button in thousandths of the viewport (347/1000, 130/1000 of 519x802 => 180,104)
RPC1=$(bun "$HERE/hub_rpc.mjs" $HPORT "$HTOK" '[{"m":"bots.browser","p":{"bot":"t1","op":"takeover"}},{"m":"bots.browser","p":{"bot":"t1","op":"click","x":347,"y":130},"wait":1500}]' 2>&1 | head -3)
echo "phone rpc: $RPC1"
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do T3=$(title); [ "$T3" = clicked:2 ] && break; sleep 0.7; done; check "a click sent through the hub (phone tap) reached the real CEF page (title '$T3')" "$([ "$T3" = clicked:2 ] && echo 1 || echo 0)" "$T3"
curl -s -m5 -H "Authorization: Bearer $(cat $HTOK)" http://127.0.0.1:$HPORT/bots/t1/screen.jpg -o "$OUT/screen2.jpg" || true
check "screen.jpg changed after the tap (the live page, not a stale still)" "$([ "$(cmp -s $OUT/screen.jpg $OUT/screen2.jpg && echo same)" != same ] && echo 1 || echo 0)" "same"
# download: real click on the link
python3 "$HERE/xinput.py" click $((WX+VX+125)) $((WY+VY+269)); sleep 3; titles > "$OUT/targets-after-download.txt"; echo "after download click: $(title)"; import -window root "$OUT/screen-download.png" 2>/dev/null || true
DLF=$(ls "$OUT"/fakehome/Downloads 2>/dev/null | head -1)
check "a download lands in ~/Downloads ($DLF)" "$([ "$DLF" = file.txt ] && grep -q downloaded-bytes "$OUT/fakehome/Downloads/file.txt" && echo 1 || echo 0)" "$DLF"
# file dialog: real click on the file input (test hook answers with test-upload.txt)
python3 "$HERE/xinput.py" click $((WX+VX+150)) $((WY+VY+380)); sleep 2.5
T4=$(title); check "file dialog answered and the page got the file (title '$T4')" "$([ "$T4" = file:test-upload.txt ] && echo 1 || echo 0)" "$T4"
# F12: Chromium's DevTools in a window of its own (a separate target in the engine's list)
python3 "$HERE/xinput.py" click $((WX+VX+300)) $((WY+VY+500)); sleep 0.5; python3 "$HERE/xinput.py" key F12; sleep 3
titles > "$OUT/targets-devtools.txt"; check "F12 opened DevTools in its own window ($(grep -ci devtools "$OUT/targets-devtools.txt") target)" "$(grep -qi devtools "$OUT/targets-devtools.txt" && echo 1 || echo 0)" "$(head -3 "$OUT/targets-devtools.txt")"
python3 "$HERE/xinput.py" key F12; sleep 1
# teach: start recording, a real native click, then the draft (the `teach.draft.t1` info the hub tells connected clients) holds a click step: one hub_rpc connection listens for 7 s after the start while the click happens
bun "$HERE/hub_rpc.mjs" $HPORT "$HTOK" '[{"m":"bots.teach","p":{"bot":"t1","op":"start"},"wait":7000}]' > "$OUT/teach-start.txt" 2>&1 & TP=$!
sleep 2.5; python3 "$HERE/xinput.py" click $((WX+VX+180)) $((WY+VY+104)); wait $TP || true
bun "$HERE/hub_rpc.mjs" $HPORT "$HTOK" '[{"m":"bots.teach","p":{"bot":"t1","op":"stop"},"wait":500}]' > "$OUT/teach-stop.txt" 2>&1 || true
TD=$(grep DRAFT "$OUT/teach-start.txt" | tail -1 | cut -c1-600)
check "teach mode recorded the native click in the hub's draft" "$(echo "$TD" | grep -q 'do.*click.*Click me' && echo 1 || echo 0)" "$TD"
# popup keeps its opener
python3 "$HERE/xinput.py" click $((WX+VX+180)) $((WY+VY+328)); sleep 7
titles > "$OUT/targets-after-popup.txt"
# the popup (a window.open target the host creates paused) loads, and its page sees window.opener: evaluated in the popup itself once it has run
for i in 1 2 3 4 5 6 7 8 9 10; do bun "$HERE/cdp_eval.mjs" $CDP 'JSON.stringify([location.pathname, !!window.opener])' > "$OUT/popup-opener.txt" 2>&1 || true; grep -q 'popup.html' "$OUT/popup-opener.txt" && break; sleep 2; done
check "window.open popup loaded and kept its opener ($(grep popup.html "$OUT/popup-opener.txt" | head -1 | cut -c1-120))" "$(grep -q 'popup.html.*true' "$OUT/popup-opener.txt" && echo 1 || echo 0)" "$(cat "$OUT/popup-opener.txt" | head -3)"
# a real layout change resizes the page: fold the sidebar (the chevron at the window's top left), measured from the change to the first painted frame of the new size
python3 "$HERE/xinput.py" click 16 59; sleep 1
wait $CP || true
python3 - "$OUT/client.log" <<'PY' > "$OUT/bench.txt"
import json,sys,re
reps=[]
for l in open(sys.argv[1],errors='ignore'):
    if '"non_white_samples"' in l:
        try: reps.append(json.loads(l[l.index('{'):]))
        except Exception: pass
resize=[r['resize_to_frame_ms'] for r in reps if r.get('resize_to_frame_ms',-1)>=0]
a,b,c=reps[-3],reps[-2],reps[-1]
fps=(b['paints']-a['paints'])/max(0.001,(b['epoch_ms']-a['epoch_ms'])/1000)
print(round(fps,1), round(c.get('click_to_pixel_ms',-1),1), round(resize[-1],1) if resize else -1)
PY
read FPS LAT RSZ < "$OUT/bench.txt"
echo "measured: ${FPS} fps painted into the gpui image (animated page, software rendering), click to changed pixel ${LAT} ms, resize to painted frame ${RSZ} ms"
check "the page animates at a usable frame rate (${FPS} fps, software GL)" "$(python3 -c "print(1 if float('${FPS:-0}')>=15 else 0)")" "$FPS"
check "input to visible change is fast (click to changed pixel ${LAT} ms)" "$(python3 -c "print(1 if 0<=float('${LAT:--1}')<400 else 0)")" "$LAT"
check "a resize reaches a painted frame quickly (${RSZ} ms)" "$(python3 -c "print(1 if 0<=float('${RSZ:--1}')<600 else 0)")" "$RSZ"
A2=$(grep 'MCPCALL' "$OUT/standin.log" | uniq | sed -n 2p | cut -c1-300 || true)
A3=$(grep 'MCPCALL' "$OUT/standin.log" | uniq | sed -n 3p | cut -c1-300 || true)
check "agent refused while the person drives" "$(echo "$A2" | grep -qi 'person has the browser' && echo 1 || echo 0)" "$A2"
check "after hand back the agent sees the person's input (typed:hello)" "$(echo "$A3" | grep -q 'typed:hello' && echo 1 || echo 0)" "$A3"
TABS=$(grep '"non_white_samples"' "$OUT/client.log" | tail -1 | grep -o '"tabs":[0-9]*' | cut -d: -f2); check "the popup became a second tab (tabs=$TABS)" "$([ "${TABS:-0}" = 2 ] && echo 1 || echo 0)" "$TABS"
echo "passed=$ok failed=$fail; artifacts $OUT"; [ $fail -eq 0 ]
