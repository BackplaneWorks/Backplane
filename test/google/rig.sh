#!/bin/bash
# Starts a temp hub (stand-ins, --no-tailscale), the mock Google (mock.py), Xvfb + i3 and the GPUI client on Settings; leaves them up (pids in $R/*.pid, `rig.sh stop`).
# Needs both fast builds in this worktree. Shots: import -window root. Clicks: python3 -I test/cef/xinput.py click X Y.
set -u; WT=$(cd "$(dirname "$0")/../.." && pwd); R=${R:-$HOME/.cache/bp-tmp/rig-google}; X=/home/h/Documents/i2cjak/backplane-gpui/build/gpui/xvfb
if [ "${1:-}" = stop ]; then for f in client i3 xvfb hub mock; do [ -f $R/$f.pid ] && kill $(cat $R/$f.pid) 2>/dev/null; done; exit 0; fi
free() { python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'; }
DN=${DN:-$(for d in $(seq 141 199); do [ -e /tmp/.X11-unix/X$d ] || [ -e /tmp/.X$d-lock ] || { echo $d; break; }; done)}; DISP=:$DN; PORT=$(free); MPORT=$(free)
rm -rf $R; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache; echo "PORT=$PORT MPORT=$MPORT DISP=$DISP" > $R/env
printf '#!/bin/sh\nexit 1\n' > $R/bin/claude; chmod +x $R/bin/claude; for n in codex grok muse; do cp $R/bin/claude $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time())
ev=[{"$":"SettingSet","key":k,"value":v,"at":N} for k,v in [("restart.continue","off"),("onboard.done","1"),("onboard.uber","1")]]
ev+=[{"$":"ThreadCreated","id":"tb","project":"","title":"Calendar","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N},{"$":"BotSet","id":"bcal","name":"Calendar","thread":"tb","look":1,"persona":"","at":N}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
json.dump([{"wait":3000},{"click":"ui:Settings"},{"wait":600000}],open(R+"/script.json","w"))
PY
python3 $WT/test/google/mock.py $MPORT & echo $! > $R/mock.pid
export LC_ALL=C BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local BACKPLANE_VAULT_KEYRING=file
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 & echo $! > $R/xvfb.pid; sleep 2
export DISPLAY=$DISP
(cd $R/proj; BACKPLANE_GOOGLE_OAUTH=http://127.0.0.1:$MPORT BACKPLANE_GOOGLE_API=http://127.0.0.1:$MPORT BACKPLANE_GOOGLE_AUTH=http://127.0.0.1:$MPORT/auth BACKPLANE_GOOGLE_CLIENT_ID=test-client.apps.googleusercontent.com BACKPLANE_GOOGLE_CLIENT_SECRET=test-client-secret \
  $WT/native/host/target/fast/backplane-hub --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1 & echo $! > $R/hub.pid); sleep 5
printf "ipc-socket $R/i3.sock\nfont pango:monospace 8\nnew_window none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
i3 -c $R/i3.conf > $R/i3.log 2>&1 & echo $! > $R/i3.pid; sleep 1
# xdg-open stand-in: the browser. It follows the sign-in link (the mock's /auth 302s to the client's own listener).
printf '#!/bin/sh\necho "$1" >> %s/opened.urls\ncurl -s -L -m10 "$1" > %s/browser.html\n' $R $R > $R/bin/xdg-open; chmod +x $R/bin/xdg-open
export DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent PATH=$R/bin:$PATH VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe BACKPLANE_GPUI_ASSETS=$WT/native/gpui/assets BACKPLANE_GPUI_RECTS=1
$WT/native/gpui/target/fast/backplane-gpui --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 1500 --theme dark > $R/client.log 2>&1 & echo $! > $R/client.pid
echo "up: $(cat $R/env)"
