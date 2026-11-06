#!/bin/sh
# One send, end to end, on a temp home (never the store copy): the GPUI client types "hello" and sends it to the Rust hub (stand-in agents only,
# loopback, isolated Xvfb display, software rendering = llvmpipe, one scope capped at 3700M, swap 0, nice 19). Checks: `send ok:true` in the
# client log, `rpc_reply turn.start ok` and a shot after the send (the user message shown, the composer empty). The fixture's events.jsonl is
# written compact (the hub's fold reads the exact `{"$":"` prefix). The client binary comes from a Boat job (never built here).
# usage: desktop-send-check.sh CLIENT_BIN ASSETS_DIR OUTNAME BOARD   ->  build/gpui/results/desktop/OUTNAME.*, build/gpui/shots/desktop/OUTNAME-after-send.png
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; CLIENT=$1; ASSETS=$2; N=${3:-plotbig}; BOARD=$4; MODES=${MODES:-current}
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
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
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
if [ "${HUBKIND:-rust}" = js ]; then $ROOT/build/gpui/backplane-serve-current --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --headless > $R/hub.log 2>&1 &
else $HUB/backplane-hub --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --window-bin $HUB/window > $R/hub.log 2>&1 &
fi
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
cat > $R/script.json <<JSON
[{"select":0},{"wait":1500},{"type":"hello"},{"send":true},{"wait":4000}]
JSON
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 25 > $OUT/$N.client.log 2>&1 &
CP=$!; wait $CP || true
cp $R/hub.log $OUT/$N.hub.log; cp $R/home/events.jsonl $OUT/$N.events.jsonl 2>/dev/null || true
'
