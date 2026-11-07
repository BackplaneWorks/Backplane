#!/bin/sh
# The GPUI 3D view on a real board made natively (kicad/pcb3d): the bare board at once, then its parts fading in as they convert.
# A private rig: the hub on a temp home (stand-in agents, loopback, port 3841, its own empty models cache so the parts convert cold),
# Xvfb on :67, i3, the client scripted to open the 3D view; the root window is captured every SHOT_MS while the parts stream in.
# usage: desktop-3d-parts.sh HUB_BIN CLIENT_BIN BOARD_DIR [NAME]   -> $BP_TMP/b3d-rig/NAME/shot-*.png, hub.log (pcb3d's timings)
# BOARD_FIRST_MS=N opens the board view first and the 3D view N ms later. GPU=0 draws with llvmpipe (the CPU raster path) instead of the NVIDIA ICD. WARM=1 keeps the models cache of the last run.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); . "$HERE/lib-bptmp.sh"
MAIN=${GPUI_MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}
HUB=$1; CLIENT=$2; BOARD_DIR=$3; N=${4:-gnss}
R=$BP_TMP/b3d-rig/$N; X=$MAIN/build/gpui/xvfb; PORT=${PORT:-3841}; DISP=${DISP:-:67}; SHOT_MS=${SHOT_MS:-120}; SHOTS=${SHOTS:-90}
[ -x "$HUB" ] && [ -x "$CLIENT" ] || { echo "no hub or client binary"; exit 2; }
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP is in use"; exit 2; }
if [ "${WARM:-}" = 1 ] && [ -d $R/fakehome/.cache ]; then mv $R/fakehome/.cache $BP_TMP/b3d-rig/.keep-cache-$N; fi
rm -rf $R; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache $R/shots
[ -d $BP_TMP/b3d-rig/.keep-cache-$N ] && { rm -rf $R/fakehome/.cache; mv $BP_TMP/b3d-rig/.keep-cache-$N $R/fakehome/.cache; }
mkdir -p $R/fakehome/.config && cp -r $HOME/.config/kicad $R/fakehome/.config/ 2>/dev/null || true  # KiCad's own paths (3D model variables), read-only copy
cp "$BOARD_DIR"/*.kicad_pcb "$BOARD_DIR"/*.kicad_pro $R/proj/
[ -d "$BOARD_DIR/../lib" ] && cp -r "$BOARD_DIR/../lib" $R/lib  # ${KIPRJMOD}/../lib models
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$MAIN/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"board","root":R+"/proj","at":NOW},{"$":"ThreadCreated","id":"t1","project":"p1","title":"3D board","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e)+"\n" for e in ev))
import os
first=os.environ.get("BOARD_FIRST_MS")  # open the board first, then the 3D view this many ms later (what a prewarm is for)
st=[{"select":0},{"wait":3000}]
if first: st+=[{"click":"rail:Board"},{"wait":int(first)}]
json.dump(st+[{"click":"rail:3D"},{"wait":60000}],open(R+"/script.json","w"))
PY
export LC_ALL=C
systemd-run --user --scope -q -- nice -n 10 sh -c '
set -eu
R='$R'; X='$X'; HUB='$HUB'; CLIENT='$CLIENT'; PORT='$PORT'; DISP='$DISP'; SHOT_MS='$SHOT_MS'; SHOTS='$SHOTS'; MAIN='$MAIN'
cleanup() { for f in client i3 xvfb hub; do [ -f $R/$f.pid ] && kill $(cat $R/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local
cd $R/proj
$HUB --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1 &
echo $! > $R/hub.pid
for i in $(seq 50); do curl -s -m1 -o /dev/null http://127.0.0.1:$PORT/hello && break; sleep 0.2; done
tr "\0" "\n" < /proc/$(cat $R/hub.pid)/environ | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "the hub has no stand-in agents: stopping"; exit 3; }
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 &
echo $! > $R/xvfb.pid; sleep 2
printf "ipc-socket $R/i3.sock\nfont pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
DISPLAY=$DISP i3 -c $R/i3.conf > $R/i3.log 2>&1 &
echo $! > $R/i3.pid; sleep 1
ICD=/usr/share/vulkan/icd.d/nvidia_icd.json; [ "${GPU:-1}" = 0 ] && ICD=/usr/share/vulkan/icd.d/lvp_icd.json
export DISPLAY=$DISP VK_ICD_FILENAMES=$ICD BACKPLANE_GPUI_ASSETS=${ASSETS:-$MAIN/build/gpui/linux/assets}
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 75 > $R/client.log 2>&1 &
echo $! > $R/client.pid
sleep ${SHOT_DELAY:-12}
i=0; while [ $i -lt $SHOTS ]; do import -window root $R/shots/shot-$(printf %03d $i)-$(date +%s%3N).png 2>/dev/null || true; i=$((i+1)); sleep $(echo "$SHOT_MS / 1000" | bc -l); done
grep -E "pcb3d|plot_frame" $R/hub.log $R/client.log | head -40
'
