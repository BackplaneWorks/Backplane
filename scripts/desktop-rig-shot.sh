#!/bin/sh
# A screenshot of the real window against a held rig's hub (a temp home): the client in a private Xvfb with its own i3, nothing on a real display.
# usage: desktop-rig-shot.sh CLIENT_BIN URL TOKEN_FILE OUT.png [SECONDS] [SCRIPT.json]   env: GPUI_DISP (:71), XVFB, BACKPLANE_GPUI_ASSETS
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; URL=$2; TOK=$3; OUT=$4; SECS=${5:-12}; SCRIPT=${6:-}
. "$ME/scripts/lib-bptmp.sh"; R=$(mktemp -d "$BP_TMP/rig-shot.XXXX")
DISP=${GPUI_DISP:-:71}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/run" "$R/fakehome/.cache"; chmod 700 "$R/run"
[ -n "$SCRIPT" ] || { echo "[{\"wait\":$((SECS * 1000 + 4000))}]" > "$R/script.json"; SCRIPT=$R/script.json; }
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$R"; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & PIDS="$! $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & PIDS="$! $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_RECTS=1
"$CLIENT" --url "$URL" --token-file "$TOK" --script "$SCRIPT" --exit-after $((SECS + 8)) > "${CLIENT_LOG:-$R/client.log}" 2>&1 & C=$!; PIDS="$C $PIDS"
sleep "$SECS"; mkdir -p "$(dirname "$OUT")"; import -window root "$OUT"; echo "shot: $OUT"
