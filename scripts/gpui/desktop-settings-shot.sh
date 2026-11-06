#!/bin/sh
# The settings dialog on a temp home (never the store copy), stand-in agents, loopback, isolated Xvfb display (llvmpipe), capped scope:
# opens Settings, prints its structure (read:settings: labels, button actions, field names, no values) and takes a shot of the first page
# and of five others. THEME=dark|light. usage: desktop-settings-shot.sh CLIENT_BIN ASSETS_DIR OUTNAME BOARD -> results/desktop/OUTNAME.*, shots/desktop/OUTNAME-s-*.png
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; CLIENT=$1; ASSETS=$2; N=${3:-plotbig}; BOARD=$4; MODES=${MODES:-current}
R=$ROOT/build/gpui/desktop-plot-cache; OUT=$ROOT/build/gpui/results/desktop; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; HUB=$ROOT/build/gpui/hub/new
FIX=$ROOT/test/gpui/files/fixtures/plot; PORT=3841; DISP=:${GPUI_DISP:-71}
mkdir -p $R $OUT $SH
if [ "${GPU:-}" = 1 ]; then nvidia-smi --query-gpu=utilization.gpu,memory.used --format=csv,noheader | tee $OUT/$N.gpu-before.txt; nvidia-smi --query-compute-apps=pid,name --format=csv,noheader >> $OUT/$N.gpu-before.txt; fi
[ -x "$CLIENT" ] || { echo "no client binary"; exit 2; }
[ ! -e /tmp/.X11-unix/X${GPUI_DISP:-71} ] || { echo "display :61 is in use"; exit 2; }
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
$ROOT/build/gpui/hub/prov/backplane-hub-full --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1 &

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
st=[{"select":0},{"wait":1500},{"click":"ui:Settings"},{"wait":3000},{"read":"settings"},{"mark":"s-first"},{"wait":1500}]
for p in ["threads","agents","devices","keys","about"]:
    st+=[{"action":["settings-page",p]},{"wait":1800},{"read":"settings"},{"mark":"s-"+p},{"wait":1500}]
json.dump(st,open("'$R'/script.json","w"))
PY
BACKPLANE_GPUI_METRICS=1 $CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 60 --theme ${THEME:-dark} > $OUT/$N.client.log 2>&1 &
CP=$!; echo $CP > $R/client.pid
for m in s-first s-threads s-agents s-devices s-keys s-about; do
  for k in $(seq 1 90); do grep -q "\"name\":\"$m\"" $OUT/$N.client.log && break; sleep 1; done
  sleep 0.5; import -window root $SH/$N-$m.png
done
for k in $(seq 1 10); do kill -0 $CP 2>/dev/null || break; sleep 1; done
kill $CP 2>/dev/null || true; wait $CP 2>/dev/null || true
cp $R/hub.log $OUT/$N.hub.log
'
