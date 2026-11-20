#!/bin/sh
# Infinite context's progress, screenshot on a seeded hub (test/e2e/memory_seed.py: the uberbot Bub, its memory part written): the Rust hub on a temp home,
# stand-in agents only, loopback, the client in a private Xvfb (nothing maps on a real display). Shots: OUTDIR/memory-chat-<theme>.png (the context chip)
# and OUTDIR/memory-settings-<theme>.png (the bot's Settings tab).
# usage: desktop-memory-shot.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR   env: GPUI_PORT (3847), GPUI_DISP (:69), THEMES ("dark"), XVFB, VK_ICD
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; OUT=$4; . "$ME/scripts/lib-bptmp.sh"; R=$(mktemp -d "$BP_TMP/memory-shot.XXXX")
PORT=${GPUI_PORT:-3847}; DISP=${GPUI_DISP:-:69}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/run" "$OUT"; chmod 700 "$R/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in grok muse codex; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
python3 "$ME/test/e2e/memory_seed.py" "$R/home/events.jsonl"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -extension GLX -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nbar {\n  mode invisible\n}\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/wm.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=${VK_ICD:-/usr/share/vulkan/icd.d/lvp_icd.json} LP_NUM_THREADS=4 BACKPLANE_GPUI_ASSETS="$ASSETS"
for th in ${THEMES:-dark}; do
  # the uberbot's chat (the context chip), then its Settings tab (the uberbot's seventh tab)
  echo '[{"wait":2500},{"keys":"ctrl-shift-u"},{"wait":9000},{"click":"btab-6","within":8000},{"wait":60000}]' > "$R/script.json"
  "$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --theme "$th" --script "$R/script.json" --exit-after 40 > "$R/client-$th.log" 2>&1 & C=$!
  sleep ${SHOT_AT:-10}; import -window root "$OUT/memory-chat-$th.png"
  sleep 6; import -window root "$OUT/memory-settings-$th.png"; kill "$C" 2>/dev/null || true; wait "$C" 2>/dev/null || true
  echo "shot $OUT/memory-chat-$th.png $OUT/memory-settings-$th.png"
done
[ -n "${KEEP:-}" ] && echo "kept $R" || rm -rf "$R"
