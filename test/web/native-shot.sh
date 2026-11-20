#!/usr/bin/env bash
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
source "$root/test/e2e/lib/own-group.sh"
fixture=$1
port=$2
out=$3
name=$4
[[ -d "$fixture/hub" && "$port" -ge 3799 && -x "$BP_WEB_GPUI" ]]
work=$(mktemp -d "$fixture/gpui-XXXXXX")
mkdir -m 700 "$work/home" "$work/run"
case "$name" in
  native-devices) steps='[{"select":0},{"open_settings":true},{"click":"set-nav:devices","within":4000},{"wait":500},{"shot":"native-devices"}]' ;;
  native-chat) steps='[{"select":0},{"wait":500},{"shot":"native-chat"}]' ;;
  *) exit 2 ;;
esac
echo "$steps" > "$work/script.json"
d=$((500 + $$ % 10000))
while [[ -e /tmp/.X11-unix/X$d || -e /tmp/.X$d-lock ]]; do ((d+=1)); done
display=:$d
trap 'set +e; own_stop' EXIT
own env LD_LIBRARY_PATH="${BP_XVFB_LIB:-}" "${BP_XVFB:-Xvfb}" "$display" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb
xpid=$!
for ((i=0; i<100; i++)); do [[ -S /tmp/.X11-unix/X$d ]] && break; sleep .05; done
kill -0 "$xpid"
cat > "$work/i3.conf" <<CONF
font pango:monospace 8
new_window none
default_border none
hide_edge_borders both
ipc-socket $work/i3.sock
CONF
own env -i PATH="$PATH" HOME="$work/home" DISPLAY="$display" XDG_RUNTIME_DIR="$work/run" i3 -c "$work/i3.conf" >"$work/i3.log" 2>&1
for ((i=0; i<100; i++)); do [[ -S "$work/i3.sock" ]] && break; sleep .05; done
[[ -S "$work/i3.sock" ]]
own python3 "$root/test/viewers/rail_shots_driver.py" watch "$display" "$work/client.log" "$work" "$out" "$work/stop"
own env -i PATH="$PATH" HOME="$work/home" XDG_CONFIG_HOME="$work/home/config" XDG_DATA_HOME="$work/home/data" XDG_CACHE_HOME="$work/home/cache" XDG_RUNTIME_DIR="$work/run" DISPLAY="$display" LC_ALL=C LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe LP_NUM_THREADS=2 BACKPLANE_GPUI_RECTS=1 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json BACKPLANE_GPUI_ASSETS="$root/assets" BACKPLANE_GPUI_SHOT_DIR="$work" "$BP_WEB_GPUI" --url "http://127.0.0.1:$port" --token-file "$fixture/hub/token" --script "$work/script.json" --exit-after 45 >"$work/client.log" 2>&1
client=$!
if ! wait "$client" || [[ ! -f "$work/$name.done" ]]; then
  python3 - "$work/client.log" <<'PYLOG'
import sys,json
for line in open(sys.argv[1]):
    if "GPUI_METRIC " in line:
        try:
            event=json.loads(line.split("GPUI_METRIC ",1)[1])
            import re
            reason=str(event.get("status",""))+" "+str(event.get("error",""))
            print("GPUI event",event.get("event"),re.sub(r'[a-zA-Z0-9_-]{16,}', '[redacted]',reason[:180]),event.get("projects",""))
        except ValueError: pass
    elif any(word in line.lower() for word in ("error","panic","failed","missing")):
        import re
        print(re.sub(r'[a-zA-Z0-9_-]{16,}', '[redacted]', line[:240]).strip())
PYLOG
  exit 1
fi
[[ -f "$work/$name.done" && -s "$out/$name.png" ]]
