#!/bin/sh
# The model menu and the composer's popup driven by real keys: the Rust hub on a temp home, stand-in agents only, loopback, the client in a private Xvfb with its own i3 (nothing maps on a
# real display), XTest input from test/e2e/picker_keys.py, which judges by the client's own `pick`/`popup` metrics and by what reached the hub's log.
# usage: desktop-picker-keys.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUTDIR]   env: GPUI_PORT (3843), GPUI_DISP (:66), XVFB (the Xvfb binary, default ~/.local/xvfb-root/usr/bin/Xvfb)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; R=${4:-$(mktemp -d /tmp/picker-keys.XXXX)}
PORT=${GPUI_PORT:-3843}; DISP=${GPUI_DISP:-:66}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
NOW=$(date +%s)
printf '{"$":"ProjectCreated","id":"p1","title":"keys","root":"%s","at":%s}\n{"$":"ThreadCreated","id":"t1","project":"p1","title":"keys thread","env":"local","branch":"","worktree":"","provider":"claude","model":"opus","at":%s}\n' "$R/proj" "$NOW" "$NOW" > "$R/home/events.jsonl"
echo '[{"select":0},{"wait":280000}]' > "$R/script.json"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS   # a private runtime dir: i3 dies on the shared one's socket, and with no window manager the client never draws
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
tr '\0' '\n' < "/proc/$H/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; exit 3; }
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS"
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 300 > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"; sleep 8
python3 "$ME/test/e2e/picker_keys.py" "$R" "$ME/test/cef/xinput.py"
