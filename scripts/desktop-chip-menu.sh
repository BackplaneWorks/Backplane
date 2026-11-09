#!/bin/sh
# The composer's chip menu driven by real mouse and keys: the Rust hub on a temp home, stand-in agents only, loopback, the client in a private Xvfb with a window manager
# of its own (nothing maps on a real display); test/e2e/chip_menu.py judges by the client's G3 layout rectangles that the menu floats and nothing else moves.
# usage: desktop-chip-menu.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUTDIR]   (Xvfb without GLX; on a machine with a real GPU run it under a no-GPU wrapper)
#   env: GPUI_PORT (3845), GPUI_DISP (:67), XVFB (the Xvfb binary, default ~/.local/xvfb-root/usr/bin/Xvfb), XVFB_LIB (its libraries, if not the system's),
#        GPUI_WM (a window manager to run instead of i3, e.g. twm), VK_ICD (the Vulkan ICD json, default lavapipe's)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; . "$ME/scripts/lib-bptmp.sh"; R=${4:-$(mktemp -d "$BP_TMP/chip-menu.XXXX")}
PORT=${GPUI_PORT:-3845}; DISP=${GPUI_DISP:-:67}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
printf '#!/bin/sh\n[ "$1" = --version ] && echo "codex-cli 0.0 (stand-in)" && exit 0\nexit 1\n' > "$R/bin/codex"; chmod +x "$R/bin/codex"   # another agent with its own model list
NOW=$(date +%s)
printf '{"$":"ProjectCreated","id":"p1","title":"chip","root":"%s","at":%s}\n{"$":"ThreadCreated","id":"t1","project":"p1","title":"chip thread","env":"local","branch":"","worktree":"","provider":"claude","model":"opus","at":%s}\n' "$R/proj" "$NOW" "$NOW" > "$R/home/events.jsonl"
echo '[{"select":0},{"wait":280000}]' > "$R/script.json"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
tr '\0' '\n' < "/proc/$H/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; exit 3; }
LD_LIBRARY_PATH=${XVFB_LIB:-} "$XV" "$DISP" -screen 0 1600x1000x24 -nolisten tcp -extension GLX -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
if [ -n "${GPUI_WM:-}" ]; then DISPLAY="$DISP" $GPUI_WM > "$R/wm.log" 2>&1 & I=$!
else printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nfloating_modifier Mod4\nfor_window [class=".*"] floating enable\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
  DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/wm.log" 2>&1 & I=$!; fi
PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=${VK_ICD:-/usr/share/vulkan/icd.d/lvp_icd.json} LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 300 > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"; sleep 8
python3 "$ME/test/e2e/chip_menu.py" "$R" "$ME/test/cef/xinput.py"
