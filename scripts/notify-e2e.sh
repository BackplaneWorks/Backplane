#!/bin/sh
# Settings, Notifications on the desktop, end to end: the hub on a temp home (stand-in agents), the GPUI client in a private Xvfb, a recording program for the desktop
# notification (BACKPLANE_GPUI_NOTIFY_CMD), real mouse clicks on the page's chips. test/e2e/notify.py drives and judges: with Needs me on a thread's ask raises a notification, with it
# switched off (by a click) it does not, and back on it does again. Pictures: $R/shots; results: $R/result.json.
# usage: notify-e2e.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUTDIR]     env: GPUI_PORT (3912), GPUI_DISP (:78)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; . "$ME/scripts/lib-bptmp.sh"; R=${4:-$(mktemp -d "$BP_TMP/notify-e2e.XXXX")}
PORT=${GPUI_PORT:-3912}; DISP=${GPUI_DISP:-:78}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in grok muse codex; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
printf '#!/bin/sh\nprintf "%%s\\n" "$*" >> %s/notified.txt\n' "$R" > "$R/bin/notify-recorder"; chmod +x "$R/bin/notify-recorder"
NOW=$(date +%s)
printf '{"$":"ProjectCreated","id":"p1","title":"board","root":"%s","at":%s}\n{"$":"ThreadCreated","id":"t1","project":"p1","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":%s}\n{"$":"ThreadCreated","id":"t2","project":"p1","title":"Second","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":%s}\n{"$":"ThreadCreated","id":"t3","project":"p1","title":"Third","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":%s}\n' "$R/proj" "$NOW" "$NOW" "$NOW" "$NOW" > "$R/home/events.jsonl"
printf '{"$":"SettingSet","key":"onboard.done","value":"1","at":%s}\n{"$":"SettingSet","key":"onboard.uber","value":"1","at":%s}\n{"$":"SettingSet","key":"restart.continue","value":"off","at":%s}\n' "$NOW" "$NOW" "$NOW" >> "$R/home/events.jsonl"
echo '[{"open_settings":true},{"action":["settings-page","notifications"]},{"mark":"page"},{"wait":280000}]' > "$R/script.json"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"; git init -q .
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
tr '\0' '\n' < "/proc/$H/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; exit 3; }
LD_LIBRARY_PATH=${XVFB_LIB:-} "$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -extension GLX -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/wm.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_NOTIFY_CMD="$R/bin/notify-recorder"
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 280 > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"; sleep 8
python3 -I "$ME/test/e2e/notify.py" "$R" "$ME/test/cef/xinput.py" "$PORT" "$ME" "$DISP"
