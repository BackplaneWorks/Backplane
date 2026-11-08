#!/bin/sh
# gpui-start.sh NAME SECONDS [ARGS...]: the LAUNCH of test/e2e/pairing/gpui.sh on Windows (Git's sh, in the desktop session): one private client,
# its settings in $WORK/rig/$RIG/config (RIG, else NAME), the steps $WORK/NAME.steps.json, its output $WORK/logs/gpui-NAME.out. BACKPLANE_HUB_BIN keeps a client run
# from a build folder from offering to install itself.
n=$1; s=$2; shift 2
cfg="$WORK/rig/${RIG:-$n}/config"; mkdir -p "$cfg" "$WORK/logs"   # RIG names the client whose settings a run uses (gpui.sh)
XDG_CONFIG_HOME="$cfg" APPDATA="$cfg" BACKPLANE_HUB_BIN="${BACKPLANE_HUB_BIN:-none}" BACKPLANE_GPUI_SCRIPT="$WORK/$n.steps.json" \
  "$GPUI" --exit-after "$s" "$@" > "$WORK/logs/gpui-$n.out" 2>&1
# after a pairing the client starts itself again with the new hub and ends (pairing.rs); that copy writes to the same log: wait for it too
# (test clients are the ones started with --exit-after, which end by themselves)
for _ in $(seq 1 $((s * 2 + 30))); do
  powershell.exe -NoProfile -Command "if (Get-CimInstance Win32_Process -Filter \"Name = 'backplane-gpui.exe'\" | ? { \$_.CommandLine -like '*--exit-after*' }) { exit 1 }" >/dev/null 2>&1 && break
  sleep 1
done
