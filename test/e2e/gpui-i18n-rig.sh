#!/bin/bash
# A private rig for looking at the GPUI window in another language: Xvfb + i3, a hub on a temp home with stand-in agents (loopback, no tailscale), the client with
# BACKPLANE_GPUI_LOCAL pointing at a settings file of its own. Never touches the person's screen or ~/.backplane*.
#   gpui-i18n-rig.sh up [auto|en|ja] [setup]   start it (setup: the window opens on the setup flow, onboard.done not seeded) (language written to the device settings file; BACKPLANE_GPUI_I18N_MISSES=$R/misses.txt collects what the catalog lacks)
#   gpui-i18n-rig.sh shot NAME         $R/shots/NAME.png        click X Y | type TEXT | key NAME | chord MOD.. KEY      real X input (test/cef/xinput.py)
#   gpui-i18n-rig.sh down              stop what `up` started (its own pids only)
# Env: BIN_HUB, BIN_CLIENT (default: this worktree's target/fast), ASSETS (the generated assets), R (default ~/.cache/bp-tmp/i18n/rig: keep it short, i3's socket path must stay under 108 bytes).
set -u
WT=$(cd "$(dirname "$0")/../.." && pwd)
R=${R:-$HOME/.cache/bp-tmp/i18n/rig}; BIN_HUB=${BIN_HUB:-$WT/native/host/target/fast/backplane-hub}; BIN_CLIENT=${BIN_CLIENT:-$WT/native/gpui/target/fast/backplane-gpui}
MAIN=${MAIN:-$(cd "$WT/../../.." && pwd)}
ASSETS=${ASSETS:-$MAIN/build/gpui/linux/assets}; XV=${XV:-$MAIN/build/gpui/xvfb}
cmd=${1:-}; shift || true
case $cmd in
up)
  lang=${1:-ja}; setup=${2:-}; rm -rf "$R"; mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/shots"
  DN=$(for d in $(seq 141 199); do [ -e /tmp/.X11-unix/X$d ] || [ -e /tmp/.X$d-lock ] || { echo $d; break; }; done); DISP=:$DN
  PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
  cp "$WT/test/files/fixtures/plot/ecc83-pp.kicad_pcb" "$R/proj/ecc83.kicad_pcb"; echo '{}' > "$R/proj/ecc83.kicad_pro"
  (cd "$R/proj" && git init -q -b main && git add -A && git -c user.email=r@x -c user.name=rig commit -qm board)
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$WT/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
  python3 - "$R" "$setup" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time()); ob=["onboard.done","1"] if sys.argv[2]!="setup" else ["onboard.step","welcome"]
ev=[{"$":"SettingSet","key":"restart.continue","value":"off","at":N},{"$":"SettingSet","key":ob[0],"value":ob[1],"at":N},{"$":"SettingSet","key":"onboard.uber","value":"1","at":N},
 {"$":"ProjectCreated","id":"p1","title":"ecc","root":R+"/proj","at":N},
 {"$":"ThreadCreated","id":"t1","project":"p1","title":"Board work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N},
 {"$":"ThreadCreated","id":"t2","project":"p1","title":"Second build","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N+1}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
  printf '{"lang":"%s"}' "$lang" > "$R/local.json"
  cat > "$R/env" <<EOF
R=$R; DISP=$DISP; PORT=$PORT
EOF
  export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local BACKPLANE_LOCK_DIR=$R
  # children start with fds 3-9 closed (a flock inherited by a rig hub would hold the lock while it lives)
  LD_LIBRARY_PATH=$XV/usr/lib/x86_64-linux-gnu "$XV/usr/bin/Xvfb" $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&- & echo $! > "$R/xvfb.pid"; sleep 2
  (cd "$R/proj" && "$BIN_HUB" --home "$R/home" --port $PORT --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&- & echo $! > "$R/hub.pid"); sleep 5
  printf "ipc-socket $R/i3.sock\nfont pango:monospace 8\nnew_window none\ndefault_border none\nhide_edge_borders both\n" > "$R/i3.conf"
  DISPLAY=$DISP i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 3>&- & echo $! > "$R/i3.pid"; sleep 1
  DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1 \
    BACKPLANE_GPUI_LOCAL=$R/local.json BACKPLANE_GPUI_I18N_MISSES=$R/misses.txt LC_ALL=C \
    "$BIN_CLIENT" --url http://127.0.0.1:$PORT --token-file "$R/home/token" --exit-after 3000 --theme dark > "$R/client.log" 2>&1 3>&- & echo $! > "$R/client.pid"
  sleep 6; echo "up: display $DISP port $PORT lang $lang rig $R" ;;
shot) . "$R/env"; DISPLAY=$DISP import -window root "$R/shots/$1.png"; echo "$R/shots/$1.png" ;;
click|type|key|chord) . "$R/env"; DISPLAY=$DISP python3 -I "$WT/test/cef/xinput.py" "$cmd" "$@" ;;
down) for f in client i3 hub xvfb; do [ -f "$R/$f.pid" ] && kill "$(cat "$R/$f.pid")" 2>/dev/null; done; echo down ;;
*) sed -n 2,8p "$0"; exit 2 ;;
esac
