#!/bin/sh
# (big-board variant, item 2) Pan/zoom/stream paint numbers, with a split into culling / lyon tessellation / submission, on a board given as BOARD; GPU=1 uses the NVIDIA ICD (nvidia-smi checked first).
# The client stops itself if the plot canvas has no height ({"expect":"canvas"}). Same caps as below.
# A real-hub check of the GPUI board and schematic viewers on this machine, kept small: the Rust hub on a temp home with the ecc83 demo
# (never the store copy), stand-in agents only, loopback, isolated display :61 (Xvfb from build/gpui/xvfb, software rendering = llvmpipe,
# NOT the GPU), every process in one scope capped at 3700M with swap 0, nice 19. The client binary comes from a Boat job (never built here).
# usage: desktop-plot-run.sh CLIENT_BIN ASSETS_DIR OUTNAME          ->  build/gpui/shots/desktop/OUTNAME-*.png, build/gpui/results/desktop/OUTNAME.*
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; CLIENT=$1; ASSETS=$2; N=${3:-plotbig}; BOARD=$4
R=$ROOT/build/gpui/desktop-plot; OUT=$ROOT/build/gpui/results/desktop; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; HUB=$ROOT/build/gpui/hub/new
FIX=$ROOT/test/files/fixtures/plot; PORT=3833; DISP=:61
mkdir -p $R $OUT $SH
if [ "${GPU:-}" = 1 ]; then nvidia-smi --query-gpu=utilization.gpu,memory.used --format=csv,noheader | tee $OUT/$N.gpu-before.txt; nvidia-smi --query-compute-apps=pid,name --format=csv,noheader >> $OUT/$N.gpu-before.txt; fi
[ -x "$CLIENT" ] || { echo "no client binary"; exit 2; }
[ ! -e /tmp/.X11-unix/X61 ] || { echo "display :61 is in use"; exit 2; }
rm -rf $R/home $R/proj $R/bin $R/fakehome; rm -f $R/*.pid; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache
cp $BOARD $R/proj/big.kicad_pcb; echo '{}' > $R/proj/big.kicad_pro
# stand-in agents: nothing may call a model
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"ecc","root":R+"/proj","at":NOW},{"$":"ThreadCreated","id":"t1","project":"p1","title":"ecc83 board","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e)+"\n" for e in ev))
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
$HUB/backplane-hub --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --window-bin $HUB/window > $R/hub.log 2>&1 &
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
ICD=/usr/share/vulkan/icd.d/lvp_icd.json; [ "${GPU:-}" = 1 ] && ICD=/usr/share/vulkan/icd.d/nvidia_icd.json
export DISPLAY=$DISP VK_ICD_FILENAMES=$ICD LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1
python3 - <<PY
import json
st=[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":15000},{"expect":"canvas"},{"record":"start","name":"pan"}]
for i in range(60): st+=[{"pan":[(14 if i%2==0 else -14),(7 if i%3 else -7)]},{"wait":40}]
st+=[{"record":"stop"},{"record":"start","name":"zoom"}]
for i in range(30): st+=[{"zoom":[(-1 if i<15 else 1),0.5,0.5]},{"wait":50}]
st+=[{"record":"stop"},{"wait":500},{"record":"start","name":"stream"},{"type":"hello"},{"send":True}]
for i in range(60): st+=[{"pan":[(14 if i%2==0 else -14),(7 if i%3 else -7)]},{"wait":40}]
st+=[{"record":"stop"},{"wait":500}]
json.dump(st,open("'$R'/script.json","w"))
PY
for MODE in run; do

/usr/bin/time -v -o $OUT/$N.$MODE.time $CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 90 > $OUT/$N.$MODE.client.log 2>&1 &
CP=$!; echo $CP > $R/client.pid
wait $CP || true
done
# the Layers panel on the big board: closed, then open (its own scroll, over the canvas, no layout jump)
cat > $R/script2.json <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":9000},{"expect":"canvas"},{"wait":2000},{"layers":true},{"wait":6000},{"layers":true,"max_h":150,"scroll":90},{"wait":8000}]
JSON
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script2.json --exit-after 40 > $OUT/$N.layers.client.log 2>&1 &
CP=$!; echo $CP > $R/client.pid
sleep 12; import -window root $SH/$N-layers-closed.png
sleep 7; import -window root $SH/$N-layers-open.png
sleep 8; import -window root $SH/$N-layers-capped.png
wait $CP || true
'
