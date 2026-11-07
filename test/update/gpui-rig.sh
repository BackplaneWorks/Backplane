#!/bin/sh
# A private GPUI rig for test/update/clients.sh: own Xvfb, i3, D-Bus, runtime and config folders, software Vulkan; nothing of the person's session.
#   WORK=<dir> RIG=<name> GPUI=<binary> gpui-rig.sh NAME SECONDS STEPS_JSON      (the rig's config, so its pairing, stays in WORK/rig/RIG between runs)
n=$1; secs=$2; steps=$3
R=$WORK/rig/$RIG; mkdir -p "$R/config" "$WORK/logs"
# the runtime folder holds sockets, whose paths may not pass 107 bytes: a short one of its own, gone at the end
RUN=$(mktemp -d "$HOME/.cache/bpr.XXXXXX"); chmod 700 "$RUN"
export XDG_RUNTIME_DIR=$RUN XDG_CONFIG_HOME=$R/config VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json BACKPLANE_GPUI_SCRIPT=$steps
unset I3SOCK DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY DISPLAY
d=$((200 + $$ % 300))
${XVFB:-Xvfb} :$d -screen 0 1400x900x24 > "$R/xvfb.log" 2>&1 & X=$!   # XVFB: an unpacked Xvfb where none is installed
sleep 1
# a window manager maps the window, without which nothing is drawn and the script never starts: i3 from I3ROOT (an unpacked bundle), else the system's
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\n' > "$R/i3.conf"
if [ -n "${I3ROOT:-}" ]; then DISPLAY=:$d LD_LIBRARY_PATH=$I3ROOT/usr/lib/x86_64-linux-gnu "$I3ROOT/usr/bin/i3" -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!
else DISPLAY=:$d i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; fi
sleep 1.5
# nothing of this machine's own Backplane: an empty environment, its own HOME (a client finds a local hub through HOME and BACKPLANE_* and would try that first)
mkdir -p "$R/home"
env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$R/home" LANG=C.UTF-8 XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" XDG_CONFIG_HOME="$XDG_CONFIG_HOME" VK_ICD_FILENAMES="$VK_ICD_FILENAMES" \
  BACKPLANE_GPUI_SCRIPT="$BACKPLANE_GPUI_SCRIPT" DISPLAY=:$d timeout $((secs+15)) dbus-run-session -- "$GPUI" --exit-after "$secs" 2>&1
rc=$?; kill $I 2>/dev/null; wait $I 2>/dev/null; kill $X 2>/dev/null; wait $X 2>/dev/null; rm -rf "$RUN"
echo "[$n] client exit $rc"
