#!/bin/sh
# A real-hub check of the GPUI board and schematic viewers on this machine, kept small: the Rust hub on a temp home with the ecc83 demo
# (never the store copy), stand-in agents only, loopback, isolated display :61 (Xvfb from build/gpui/xvfb, software rendering = llvmpipe,
# NOT the GPU), every process in one scope capped at 3700M with swap 0, nice 19. The client binary comes from a Boat job (never built here).
# usage: desktop-plot-run.sh CLIENT_BIN ASSETS_DIR OUTNAME          ->  build/gpui/shots/desktop/OUTNAME-*.png, build/gpui/results/desktop/OUTNAME.*
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; CLIENT=$1; ASSETS=$2; N=${3:-plot}
R=$ROOT/build/gpui/desktop-plot-$N; OUT=$ROOT/build/gpui/results/desktop; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; HUB=$ROOT/build/gpui/hub/new
FIX=$ROOT/test/gpui/files/fixtures/plot; PORT=${GPUI_PORT:-3833}; DISP=${GPUI_DISP:-:61}
mkdir -p $R $OUT $SH
[ -x "$CLIENT" ] || { echo "no client binary"; exit 2; }
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP is in use (set GPUI_DISP=:6N)"; exit 2; }
rm -rf $R/home $R/proj $R/bin $R/fakehome; rm -f $R/*.pid; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache
cp $FIX/ecc83-pp.kicad_pcb $FIX/ecc83-pp.kicad_sch $R/proj/; echo '{}' > $R/proj/ecc83-pp.kicad_pro; mkdir -p $R/proj/mech; cp $ROOT/test/gpui/files/fixtures/mesh/bracket.step $R/proj/mech/
# stand-in agents: nothing may call a model
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"ecc","root":R+"/proj","at":NOW},{"$":"ThreadCreated","id":"t1","project":"p1","title":"ecc83 board","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
if __import__("os").environ.get("GPUI_LONG"):
    long=" ".join("word%d" % i for i in range(220))+"\n\nsecond paragraph with a path /home/h/Documents/x.rs and https://example.com/a?b=1 here."
    ev+=[{"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":long,"at":NOW},{"$":"MessagePosted","thread":"t1","msg":"m2","role":"assistant","text":"The reply right below the long message.","at":NOW+1},{"$":"MessagePosted","thread":"t1","msg":"m3","role":"user","text":"short follow up","at":NOW+2},{"$":"MessagePosted","thread":"t1","msg":"m4","role":"assistant","text":"Final reply.","at":NOW+3}]
if __import__("os").environ.get("GPUI_THREADS") or __import__("os").environ.get("GPUI_PROJECTS"): ev+=[{"$":"ThreadCreated","id":t,"project":"p1","title":n,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW} for t,n in (("t2","beta"),("t3","gamma"))]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))   # compact, as the hub writes it: its agents world reads lines by pattern
PY
export LC_ALL=C
# one scope for everything; the inner shell stops what it started (only its own pids) on any exit
systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- nice -n 19 sh -c '
set -eu
cleanup() { for f in client i3 xvfb hub; do [ -f '$R'/$f.pid ] && kill $(cat '$R'/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
R='$R'; ROOT='$ROOT'; X='$X'; HUB='$HUB'; CLIENT='$CLIENT'; ASSETS='$ASSETS'; N='$N'; PORT='$PORT'; DISP='$DISP'; OUT='$OUT'; SH='$SH'
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local
cd $R/proj
$HUB/backplane-hub --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --policy-bin ${GPUI_POLICY:-$ROOT/build/gpui/slim/bp-policy-page} --window-bin $HUB/window > $R/hub.log 2>&1 &
echo $! > $R/hub.pid; sleep 5
HP=$(cat $R/hub.pid)
# the hub must run with the stand-in agents before any turn
tr "\0" "\n" < /proc/$HP/environ | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub (pid $HP): stopping"; kill $HP; exit 3; }
echo "hub pid $HP environ ok: $(tr "\0" "\n" < /proc/$HP/environ | grep "^BACKPLANE_AGENTS=")"; curl -s -m3 -o /dev/null -w "hello %{http_code}\n" http://127.0.0.1:$PORT/hello
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 &
echo $! > $R/xvfb.pid; sleep 2
printf "font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
DISPLAY=$DISP i3 -c $R/i3.conf > $R/i3.log 2>&1 &
echo $! > $R/i3.pid; sleep 1
export DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1
if [ -n "${GPUI_PROJECTS:-}" ]; then cat > $R/script.json <<JSON
[{"wait":1500},{"click":"ui:Search"},{"wait":800},{"field":["pq","gam"]},{"wait":2000},{"click":"ui:Add project"},{"wait":2500},{"field":["picker","~/"]},{"wait":3000},{"click":"folders-cancel"},{"wait":2500},{"field":["pq",""]},{"wait":2500}]
JSON
elif [ -n "${GPUI_LONG:-}" ]; then cat > $R/script.json <<JSON
[{"click":"thread:t1"},{"wait":14000}]
JSON
elif [ -n "${GPUI_THREADS:-}" ]; then cat > $R/script.json <<JSON
[{"click":"thread:t1"},{"wait":1500},{"click":"thread-more"},{"wait":1200},{"click":"thread-more"},{"wait":500},{"click":"tool-0"},{"wait":1500},{"action":["row-rename","t1"]},{"wait":1500},{"field":["title","alpha renamed"]},{"wait":300},{"click":"rename-yes"},{"wait":2000},{"action":["row-rename","t2"]},{"wait":1500},{"click":"rename-no"},{"wait":1500},{"action":["row-delete","t3"]},{"wait":1500},{"click":"delete-no"},{"wait":1500},{"action":["row-delete","t3"]},{"wait":1500},{"click":"delete-yes"},{"wait":2500},{"action":["row-delete","t1"]},{"wait":1500},{"click":"delete-yes"},{"wait":2500}]
JSON
elif [ -n "${GPUI_DAILY:-}" ]; then cat > $R/script.json <<JSON
[{"select":0},{"wait":1500},{"popup":"/"},{"wait":800},{"type":" "},{"type":"x"},{"wait":300},{"read":"composer"},{"wait":500},{"type":"approve:one"},{"wait":300},{"send":true},{"wait":5000},{"wait":100},{"click":"ask:0:0"},{"wait":4000},{"click":"ui:Settings"},{"wait":2500},{"click":"ui:Settings"},{"wait":1500},{"click":"project:p1"},{"wait":2500}]
JSON
elif [ -n "${GPUI_ATTACH:-}" ]; then cp "$GPUI_ATTACH" $R/attach.png; cat > $R/script.json <<JSON
[{"select":0},{"wait":1500},{"attach":"$R/attach.png"},{"wait":2500},{"type":"see attach"},{"wait":2500},{"send":true},{"wait":2500}]
JSON
else
cat > $R/script.json <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":6000},{"click":"rail:Schematic"},{"wait":6000},{"click":"rail:3D"},{"wait":8000},{"click":"rail:Mech"},{"wait":3000},{"click":"mech-part-0"},{"wait":9000}]
JSON
fi
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 80 > $OUT/$N.client.log 2>&1 &
echo $! > $R/client.pid
if [ -n "${GPUI_PROJECTS:-}" ]; then sleep 5; import -window root $SH/$N-search.png; sleep 3; import -window root $SH/$N-add.png; sleep 5; import -window root $SH/$N-picked.png; sleep 4; import -window root $SH/$N-cleared.png; elif [ -n "${GPUI_LONG:-}" ]; then sleep 9; import -window root $SH/$N-long.png; elif [ -n "${GPUI_THREADS:-}" ]; then sleep 4; import -window root $SH/$N-more.png; sleep 6; import -window root $SH/$N-rename.png; sleep 7; import -window root $SH/$N-delete.png; sleep 10; import -window root $SH/$N-end.png; elif [ -n "${GPUI_DAILY:-}" ]; then sleep 9; import -window root $SH/$N-ask.png; sleep 5; import -window root $SH/$N-after.png; sleep 3; import -window root $SH/$N-settings.png; sleep 3; import -window root $SH/$N-home.png; elif [ -n "${GPUI_ATTACH:-}" ]; then sleep 5; import -window root $SH/$N-attach.png; sleep 2; import -window root $SH/$N-attach-sent.png; else sleep 7; import -window root $SH/$N-board.png
sleep 6; import -window root $SH/$N-schematic.png
sleep 8; import -window root $SH/$N-3d.png
sleep 9; import -window root $SH/$N-mech.png; fi
wait $(cat $R/client.pid) || true
if [ -n "${GPUI_THREADS:-}" ]; then echo "events:"; grep -E "Thread(Created|Renamed|Deleted|Pinned|Archived)" $R/home/events.jsonl | cut -c1-200; grep -E "rpc_sent|rpc_refused" $OUT/$N.client.log | tail -20; fi
if [ -n "${GPUI_DAILY:-}" ]; then echo "events:"; grep -E "(MessagePosted|Ask[A-Za-z]*|TurnChanged|ToolCalled|PermissionAsked)" $R/home/events.jsonl | cut -c1-220 | tail -8; fi
if [ -n "${GPUI_ATTACH:-}" ]; then echo "attachments on the hub:"; ls -la $R/home/attachments/*/ 2>/dev/null | tail -3; echo "messages:"; grep MessagePosted $R/home/events.jsonl | tail -2 | cut -c1-300; fi
'
