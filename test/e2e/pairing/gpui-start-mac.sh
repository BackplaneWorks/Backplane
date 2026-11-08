#!/bin/sh
# gpui-start-mac.sh NAME SECONDS [ARGS...]: the LAUNCH of test/e2e/pairing/gpui.sh on macOS (a logged-in GUI session): one private client, its settings in $WORK/rig/${RIG:-NAME}/config
# (XDG_CONFIG_HOME: the client's config_base honours it on macOS too, so hubs/*.token and hubs.json land where gpui.sh looks), the steps $WORK/NAME.steps.json, its output
# $WORK/logs/gpui-NAME.out. BACKPLANE_HUB_BIN keeps a client run from a build folder from offering to install itself.
n=$1; s=$2; shift 2
cfg="$WORK/rig/${RIG:-$n}/config"; mkdir -p "$cfg" "$WORK/logs"
XDG_CONFIG_HOME="$cfg" BACKPLANE_HUB_BIN="${BACKPLANE_HUB_BIN:-none}" BACKPLANE_GPUI_SCRIPT="$WORK/$n.steps.json" \
  "$GPUI" --exit-after "$s" "$@" > "$WORK/logs/gpui-$n.out" 2>&1
# after a pairing the client starts itself again with the new hub and ends (pairing.rs); that copy writes to the same log: wait for it too
for _ in $(seq 1 $((s * 2 + 30))); do pgrep -f 'backplane-gpui.*--exit-after' >/dev/null 2>&1 || break; sleep 1; done
