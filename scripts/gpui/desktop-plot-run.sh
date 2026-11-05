#!/bin/sh
# A real-hub check of the GPUI board and schematic viewers on this machine, kept small: the Rust hub on a temp home with the ecc83 demo
# (never the store copy), stand-in agents only, loopback, isolated display :61 (Xvfb from build/gpui/xvfb, software rendering = llvmpipe,
# NOT the GPU), every process in one scope capped at 3700M with swap 0, nice 19. The client binary comes from a Boat job (never built here).
# usage: desktop-plot-run.sh CLIENT_BIN ASSETS_DIR OUTNAME          ->  build/gpui/shots/desktop/OUTNAME-*.png, build/gpui/results/desktop/OUTNAME.*
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; CLIENT=$1; ASSETS=$2; N=${3:-plot}
R=$ROOT/build/gpui/desktop-plot; OUT=$ROOT/build/gpui/results/desktop; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; HUB=$ROOT/build/gpui/hub/new
DEMO=/home/h/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos/ecc83; PORT=3833; DISP=:61
mkdir -p $R $OUT $SH
[ -x "$CLIENT" ] || { echo "no client binary"; exit 2; }
[ ! -e /tmp/.X11-unix/X61 ] || { echo "display :61 is in use"; exit 2; }
rm -rf $R/home $R/proj $R/bin; mkdir -p $R/home $R/proj $R/bin
cp -r $DEMO/ecc83-pp.kicad_pcb $DEMO/ecc83-pp.kicad_pro $DEMO/ecc83-pp.kicad_sch $R/proj/
# stand-in agents: nothing may call a model
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys
R=sys.argv[1]
ev=[{"$":"ProjectCreated","id":"p1","title":"ecc","root":R+"/proj","at":1},{"$":"ThreadCreated","id":"t1","project":"p1","title":"ecc83 board","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e)+"\n" for e in ev))
PY
export LC_ALL=C
cleanup() { for f in client hub i3 xvfb; do [ -f $R/$f.pid ] && kill $(cat $R/$f.pid) 2>/dev/null || true; done; }
trap cleanup EXIT
# one scope for everything
exec systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- nice -n 19 sh -c '
set -eu
R='$R'; ROOT='$ROOT'; X='$X'; HUB='$HUB'; CLIENT='$CLIENT'; ASSETS='$ASSETS'; N='$N'; PORT='$PORT'; DISP='$DISP'; OUT='$OUT'; SH='$SH'
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream
cd $R/proj
$HUB/backplane-hub --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --policy-bin $ROOT/build/gpui/slim/bp-policy-page --window-bin $HUB/window > $R/hub.log 2>&1 &
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
cat > $R/script.json <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":6000},{"wait":100},{"click":"rail:Schematic"},{"wait":6000}]
JSON
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 40 > $OUT/$N.client.log 2>&1 &
echo $! > $R/client.pid
sleep 14; import -window root $SH/$N-board.png
sleep 9; import -window root $SH/$N-schematic.png
wait $(cat $R/client.pid) || true
'
