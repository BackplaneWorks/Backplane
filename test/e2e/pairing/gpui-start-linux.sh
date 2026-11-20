#!/bin/sh
# gpui-start-linux.sh NAME SECONDS [ARGS...]: the LAUNCH of gpui-form.sh and gpui.sh on Linux: one private client (config and home under $WORK/rig/${RIG:-NAME}), the steps
# $WORK/NAME.steps.json, its output $WORK/logs/gpui-NAME.out. The display is the caller's (a private Xvfb or Xwayland: DISPLAY, and VK_ICD_FILENAMES for a software Vulkan).
n=$1; s=$2; shift 2
r="$WORK/rig/${RIG:-$n}"; mkdir -p "$r/config" "$r/home" "$WORK/logs"
env XDG_CONFIG_HOME="$r/config" HOME="$r/home" BACKPLANE_HUB_BIN="${BACKPLANE_HUB_BIN:-none}" BACKPLANE_GPUI_SCRIPT="$WORK/$n.steps.json" \
  "$GPUI" --exit-after "$s" "$@" > "$WORK/logs/gpui-$n.out" 2>&1
sleep 2
