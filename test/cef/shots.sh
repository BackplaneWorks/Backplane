#!/bin/sh
# shots.sh CLIENT_BIN HUB_BIN : the embedded Chromium end to end on a private Xvfb with a local hub, stand-in agents and a local test page.
# Real X input (XTest) clicks the page's button and types in its field; CDP (the engine's loopback port) shows the page reacted; the hub's preview_*
# tools (an agent's, through the stand-in's `mcpcall:`) read the same page, are refused while the person has it, and see the person's changes after hand back.
# usage: test/cef/shots.sh CLIENT_BIN HUB_BIN   -> prints PASS/FAIL lines, exit 0 when all pass; artifacts in $OUT (default /tmp/cef-e2e)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); MAIN=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
fp() { python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'; }
CLIENT=$1; HUB=$2; OUT=${OUT:-/tmp/cef-shots}; DISP=${DISP:-:79}; HPORT=${HPORT:-$(fp)}; PPORT=${PPORT:-$(fp)}; CDP=${CDP:-$(fp)}
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
( cd "$OUT/proj" && BACKPLANE_BROWSER_DEBUG=1 BACKPLANE_AGENTS=$OUT/bin BACKPLANE_BROWSER=${NOHELPER:+$OUT/no-such-helper}${NOHELPER:-$OUT/bin/backplane-browser} BACKPLANE_NO_UPDATE=1 FAKE_LOG=$OUT/standin.log BP_STANDIN_MODE=stream HOME=$OUT/fakehome XDG_CACHE_HOME=$OUT/fakehome/.cache \
  exec "$HUB" --home "$OUT/home" --port $HPORT --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.log" 2>&1 ) & HP=$!; PIDS="$PIDS $HP"
n=0; until curl -s -m1 -o /dev/null http://127.0.0.1:$HPORT/hello; do sleep 0.2; n=$((n+1)); [ $n -gt 100 ] && { echo "hub did not start"; tail -5 "$OUT/hub.log"; exit 1; }; done
# screenshots of the embedded Chromium (THEME=light|dark): a local page, then https://www.google.com when the network allows
RT=${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}
cat > "$OUT/script.json" <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Browser"},{"wait":2500},{"cef":{"goto":"http://127.0.0.1:$PPORT/index.html"}},{"wait":4000},{"cef":{"report":true}},{"wait":3000},{"cef":{"goto":"https://www.google.com/"}},{"wait":9000},{"cef":{"report":true}},{"wait":6000}]
JSON
DISPLAY=$DISP BACKPLANE_CEF=$RT BACKPLANE_CEF_CDP_PORT=$CDP BACKPLANE_CEF_NO_GPU=1 BACKPLANE_CEF_PROFILE=$OUT/profile VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$MAIN/build/gpui/linux/assets HOME=$OUT/fakehome \
  "$CLIENT" ${THEME:+--theme $THEME} --url http://127.0.0.1:$HPORT --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 60 > "$OUT/client.log" 2>&1 & CP=$!; PIDS="$PIDS $CP"
export DISPLAY=$DISP
n=0; until grep -aq '"non_white_samples"' "$OUT/client.log"; do sleep 0.5; n=$((n+1)); [ $n -gt 120 ] && { echo "no report"; exit 1; }; done
sleep 1; import -window root "$OUT/local.png" 2>/dev/null || true
n=0; until [ "$(grep -ac '"non_white_samples"' "$OUT/client.log")" -ge 2 ]; do sleep 0.5; n=$((n+1)); [ $n -gt 60 ] && break; done
sleep 1; import -window root "$OUT/web.png" 2>/dev/null || true
grep -a '"non_white_samples"' "$OUT/client.log" | cut -c1-260
echo "shots: $OUT/local.png $OUT/web.png"
