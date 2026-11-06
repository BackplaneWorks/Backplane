#!/bin/bash
# Time a hub's start on a COPY of a log, under a memory cap, with stand-in agents and the copy guard on. Prints the hub's own phase lines,
# the time to /hello and the idle RSS. The home is yours to throw away: pass a folder holding a copy of events.jsonl (never the live store).
#   scripts/gpui/startup-bench.sh HUB_BIN HOME_COPY [PORT=3799] [CAP=3700M]
# With CLIENT_BIN=<backplane-gpui> (assets beside it, or ASSETS=dir) it also opens the window on a private Xvfb display (DISPLAY_NUM, default 87) with an
# empty-config i3 and reads the client's own GPUI_METRIC lines: first_frame and usable (ms from the client's process start), plus a screenshot check that
# the screen is not blank. A window needs a window manager: without one GPUI never gets a frame callback and paints nothing. XVFB_ROOT defaults to ~/.local/xvfb-root.
set -u
HUB=${1:?HUB_BIN}; H=${2:?HOME_COPY}; PORT=${3:-3799}; CAP=${4:-3700M}
case "$H" in "$HOME/.backplane-bend"*|"$HOME/.backplane-gpui") echo "refusing: $H looks like a live home"; exit 2;; esac
SW=$(mktemp -d /tmp/bp-startup-bench-XXXX); mkdir -p "$SW/bin" "$SW/fakehome" "$SW/proj"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$SW/bin/$n"; chmod +x "$SW/bin/$n"; done
rm -f "$H/hub.lock"; t0=$(date +%s.%N)
( cd "$SW/proj" && exec setsid env -i PATH="$PATH" LC_ALL=C XDG_RUNTIME_DIR="/run/user/$(id -u)" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u)/bus" \
    HOME="$SW/fakehome" XDG_CACHE_HOME="$SW/fakehome/.cache" BACKPLANE_COPY=1 BACKPLANE_AGENTS="$SW/bin" BACKPLANE_NO_UPDATE=1 BACKPLANE_PASSIVE=1 BP_STANDIN_MODE=stream \
    systemd-run --user --scope -q -p "MemoryMax=$CAP" -p MemorySwapMax=0 -- env -u DBUS_SESSION_BUS_ADDRESS -u XDG_RUNTIME_DIR "$HUB" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$SW/hub.log" 2>&1 < /dev/null ) &
for i in $(seq 1 1200); do curl -sf -m1 "http://127.0.0.1:$PORT/hello" > /dev/null 2>&1 && { echo "/hello after $(echo "$(date +%s.%N) - $t0" | bc) s (poll step 0.05 s, includes the cap's scope start)"; break; }; sleep 0.05; done
if [ -n "${CLIENT_BIN:-}" ]; then
  N=${DISPLAY_NUM:-87}; XV=${XVFB_ROOT:-$HOME/.local/xvfb-root}; A=${ASSETS:-$(dirname "$CLIENT_BIN")/assets}
  [ ! -e "/tmp/.X11-unix/X$N" ] || { echo "display :$N exists: set DISPLAY_NUM"; }
  ( LD_LIBRARY_PATH=$XV/usr/lib/x86_64-linux-gnu exec "$XV/usr/bin/Xvfb" ":$N" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb ) > "$SW/xvfb.log" 2>&1 < /dev/null & XP=$!
  sleep 1.5; mkdir -p "$SW/wm"; printf '# empty\n' > "$SW/wm/i3.conf"
  ( DISPLAY=":$N" HOME="$SW/wm" exec i3 -c "$SW/wm/i3.conf" ) > /dev/null 2>&1 < /dev/null & IP=$!; sleep 1
  ( DISPLAY=":$N" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json BACKPLANE_GPUI_ASSETS="$A" exec "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$H/token" ) > "$SW/client.log" 2>&1 < /dev/null & CP=$!
  sleep 4
  DISPLAY=":$N" import -window root "$SW/screen.png" 2>/dev/null
  colors=$(python3 -W ignore -c "from PIL import Image; im=Image.open('$SW/screen.png').convert('RGB'); print(len(set(im.resize((144,90)).getdata())))" 2>/dev/null || echo 0)
  grep -h 'GPUI_METRIC.*"event":"\(gpu\|first_frame\|usable\)"' "$SW/client.log" | sed 's/^GPUI_METRIC //; s/"driver_info":"[^"]*",//'
  echo "screenshot $SW/screen.png: $colors distinct colours at 144x90 ($( [ "$colors" -gt 20 ] && echo painted || echo BLANK ))"
  kill "$CP" "$IP" "$XP" 2>/dev/null
fi
sleep 1; pid=$(cut -d' ' -f1 "$H/hub.lock" 2>/dev/null)
grep -E '^startup:' "$SW/hub.log"; [ -z "$pid" ] || { grep VmRSS "/proc/$pid/status"; kill "$pid"; }
echo "log: $SW/hub.log"
