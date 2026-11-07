#!/bin/bash
set -u
cleanup() { for f in client i3 xvfb hub; do [ -f $R/$f.pid ] && kill $(cat $R/$f.pid) 2>/dev/null; done; sleep 1; }
trap cleanup EXIT INT TERM
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local BACKPLANE_LOCK_DIR=$R
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 & echo $! > $R/xvfb.pid; sleep 2
export DISPLAY=$DISP
cd $R/proj; env -u BACKPLANE_KICAD_CLI BACKPLANE_KICAD_NO_SYSTEM=1 BACKPLANE_KICAD_INSTALLER=$D/standin.sh RIG=$R $HUBBIN --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1 & echo $! > $R/hub.pid; sleep 5
curl -s -m3 -o /dev/null -w "hello %{http_code}\n" http://127.0.0.1:$PORT/hello
printf "ipc-socket $R/i3.sock\nfont pango:monospace 8\nnew_window none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
i3 -c $R/i3.conf > $R/i3.log 2>&1 & echo $! > $R/i3.pid; sleep 1
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 140 --theme dark > $R/client.log 2>&1 & echo $! > $R/client.pid
w() { for k in $(seq 1 150); do grep -q "\"name\":\"$1\"" $R/client.log && return 0; sleep 1; done; echo "no mark $1"; return 1; }
for m in s-row s-ask s-cancel s-running s-failed s-running2 s-done; do w $m && { sleep 0.5; import -window root $SH/$m.png; echo "shot $m"; }; done
sleep 2; grep -o '"t":"click"[^}]*}\|"name":"click"[^}]*}' $R/client.log | head -20
