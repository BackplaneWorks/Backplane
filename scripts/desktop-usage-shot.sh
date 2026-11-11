#!/bin/sh
# The composer's Context panel, screenshot on a seeded hub (test/e2e/usage_seed.py: threads at 10%, 64% and 93% of their context, the uberbot's chat with its
# memories, a past handover and mailroom lanes): the Rust hub on a temp home, stand-in agents only, loopback, the client in a private Xvfb (nothing maps on a
# real display). One shot per (target, theme): OUTDIR/<name>-<theme>.png.
# usage: desktop-usage-shot.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR [TARGETS]   TARGETS: space-separated name=step pairs, a step being select:N or bot:ID
#   (default "bot=bot:bub thread-low=select:0 thread-mid=select:1 thread-full=select:2"; the bot first: a session that ran before may wake it); env: GPUI_PORT (3846), GPUI_DISP (:68), THEMES ("dark light"), XVFB, VK_ICD
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; OUT=$4; . "$ME/scripts/lib-bptmp.sh"; R=$(mktemp -d "$BP_TMP/usage-shot.XXXX")
TARGETS=${5:-"bot=bot:bub thread-low=select:0 thread-mid=select:1 thread-full=select:2"}
PORT=${GPUI_PORT:-3846}; DISP=${GPUI_DISP:-:68}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/run" "$OUT"; chmod 700 "$R/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in grok muse codex; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
python3 "$ME/test/e2e/usage_seed.py" "$R/home/events.jsonl" "$R/proj"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -extension GLX -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nbar {\n  mode invisible\n}\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/wm.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=${VK_ICD:-/usr/share/vulkan/icd.d/lvp_icd.json} LP_NUM_THREADS=4 BACKPLANE_GPUI_ASSETS="$ASSETS"
for th in ${THEMES:-dark light}; do for t in $TARGETS; do
  name=${t%%=*}; step=${t#*=}
  case $step in select:*) go="{\"select\":${step#select:}}";; bot:*) go="{\"keys\":\"ctrl-shift-u\"}";; esac
  echo "[{\"wait\":2500},$go,{\"wait\":3000},{\"click\":\"ui:Context\"},{\"wait\":60000}]" > "$R/script.json"
  "$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --theme "$th" --script "$R/script.json" --exit-after 40 > "$R/client-$name-$th.log" 2>&1 & C=$!
  sleep ${SHOT_AT:-9}; import -window root "$OUT/$name-$th.png"; kill "$C" 2>/dev/null || true; wait "$C" 2>/dev/null || true
  echo "shot $OUT/$name-$th.png"
done; done
[ -n "${KEEP:-}" ] && echo "kept $R" || rm -rf "$R"
