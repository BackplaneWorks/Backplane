#!/bin/bash
# A private rig for the sidebar rows: own Xvfb display, temp home and HOME, stand-in agents, loopback hub. Stays up until $R/stop exists.
# Drive it with d.sh (real X mouse and keys through test/cef/xinput.py; targets from the client's `controls` rect metric).
set -u
WT=${WT:-$(cd "$(dirname "$0")/../.." && pwd)}; MAIN=${MAIN:-${WT%%/.backplane/worktrees/*}}  # the checkout that holds build/gpui/xvfb
HUBBIN=$WT/native/host/target/fast/backplane-hub; CLIENT=$WT/native/gpui/target/fast/backplane-gpui; ASSETS=$(ls -d $HOME/.backplane-gpui-dev/builds/*/assets | tail -1)
R=$HOME/.cache/bp-tmp/sb; X=$MAIN/build/gpui/xvfb; HERE=$(dirname "$(readlink -f "$0")")
DN=$(for d in $(seq 151 199); do [ -e /tmp/.X11-unix/X$d ] || [ -e /tmp/.X$d-lock ] || { echo $d; break; }; done); DISP=:$DN
PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
rm -rf $R; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache $R/fakehome/Documents/boards
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun || echo /home/h/.bun/bin/bun)" "$WT/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 -I $HERE/seed.py $R
echo "DISP=$DISP PORT=$PORT R=$R" > $R/env; cat $R/env
pids=()
cleanup() { for p in "${pids[@]}"; do kill $p 2>/dev/null; done; sleep 1; }
trap cleanup EXIT INT TERM
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local BACKPLANE_LOCK_DIR=$R LC_ALL=C
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 & pids+=($!); sleep 2
export DISPLAY=$DISP
(cd $R/proj && exec $HUBBIN --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1) & pids+=($!); sleep 5
curl -s -m3 -o /dev/null -w "hello %{http_code}\n" http://127.0.0.1:$PORT/hello
printf "ipc-socket $R/i3.sock\nfont pango:monospace 8\nnew_window none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
i3 -c $R/i3.conf > $R/i3.log 2>&1 & pids+=($!); sleep 1
start_client() {
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1 \
    $CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --theme dark > $R/client.log 2>&1 & cpid=$!; echo $cpid > $R/client.pid
}
start_client
for i in $(seq 1 10800); do [ -e $R/stop ] && break; kill -0 $cpid 2>/dev/null || { [ -e $R/restart-client ] && { rm -f $R/restart-client; mv $R/client.log $R/client.$i.log; start_client; } ; }; sleep 1; done
kill $cpid 2>/dev/null; echo "rig stopped"
