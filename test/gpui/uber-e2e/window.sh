#!/bin/bash
# window.sh RUN WINDOW_BIN ASSETS DISPLAY_NUM SCRIPT.json OUT.png SECONDS: the GPUI window on a private Xvfb against the run's hub; prints the panic count and the metrics' events only.
set -u
R=$1; W=$2; A=$3; N=$4; S=$5; O=$6; D=${7:-20}; X=${XVFB:-/tmp/xvfb-root/usr/bin/Xvfb}
LD_LIBRARY_PATH=/tmp/xvfb-root/usr/lib/x86_64-linux-gnu "$X" ":$N" -screen 0 1400x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 &
XP=$!; sleep 2; kill -0 "$XP" || { echo "no Xvfb"; tail -3 "$R/xvfb.log"; exit 1; }
# a window manager on this display only: without one the window never gets its first frame
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > "$R/i3.conf"
env -u I3SOCK DISPLAY=":$N" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 &
IP=$!; sleep 1
env -i HOME="$R/fakehome" PATH=/usr/bin:/bin DISPLAY=":$N" XDG_RUNTIME_DIR="$R/fakehome" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$A" BACKPLANE_GPUI_RECTS=1 RUST_BACKTRACE=1 \
  "$W" --url "http://127.0.0.1:${E2E_PORT:-3811}" --token-file "$R/home/token" --script "$S" > "$R/window.log" 2>&1 &
WP=$!; for i in $(seq 1 "$D"); do kill -0 "$WP" 2>/dev/null || break; sleep 1; done
alive=no; kill -0 "$WP" 2>/dev/null && alive=yes
DISPLAY=":$N" import -window root "$O" 2>/dev/null
kill "$WP" 2>/dev/null; sleep 0.5; kill "$IP" 2>/dev/null; kill "$XP"
echo "window alive at the end: $alive; panics: $(grep -c 'panicked' "$R/window.log"); log lines: $(wc -l < "$R/window.log")"
grep -a 'panicked' -A3 "$R/window.log" | cut -c1-200 | head -12
