#!/bin/sh
# usage: desktop-shot.sh NAME SCRIPT.json SECONDS_TO_SHOT [ICD]   -> build/gpui/shots/desktop/NAME.png (Xvfb :61, isolated)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); N=$1; S=$2; D=$3; ICD=${4:-lvp}
cd $ROOT/build/gpui/linux
env BACKPLANE_GPUI_RECTS=1 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/${ICD}_icd.json ${WSI:+MESA_VK_WSI_DEBUG=$WSI} DISPLAY=${DISP:-:61} ./backplane-gpui --url http://127.0.0.1:3821 --token-file ../desktop-hub/home/token --script $ROOT/test/gpui/$S --exit-after $((D+6)) > $ROOT/build/gpui/results/desktop/$N.log 2>&1 &
P=$!; sleep $D; DISPLAY=${DISP:-:61} import -window root $ROOT/build/gpui/shots/desktop/$N.png; wait $P || true
grep -h GPUI_METRIC $ROOT/build/gpui/results/desktop/$N.log | cut -c1-200
