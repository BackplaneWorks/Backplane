#!/bin/sh
# gpui_flow.sh : capped desktop test (3.5 GiB hub, 2 GiB GPUI, display :61, synthetic log, stand-in agents only).
# Opens a thread in the GPUI binary against backplane-hub, types and sends; prints the GPUI_METRIC lines.
# Needs build/gpui/hub/backplane-hub-full, build/gpui/hub/window, build/gpui/slim/bp-policy-serve-O2-v3, build/gpui/linux/backplane-gpui.
R=/home/h/Documents/i2cjak/backplane-gpui; D=$R/build/gpui/hub-run/gpui-flow; mkdir -p /tmp/p0
rm -rf $D; mkdir -p $D/home $D/bin; cp $R/test/hub/fixtures/pages.jsonl $D/home/events.jsonl
bun -e 'import {makeStandins} from "'$R'/test/perf/lib.mjs"; makeStandins(process.argv[1]+"/bin")' $D
BACKPLANE_AGENTS=$D/bin BACKPLANE_NO_UPDATE=1 BACKPLANE_POLICY_BIN=$R/build/gpui/slim/bp-policy-serve-O2-v3 BACKPLANE_WINDOW_BIN=$R/build/gpui/hub/window FAKE_SECS=3 FAKE_PER=2 FAKE_CYCLE=5 \
  timeout 100 systemd-run --user --scope -q -p MemoryMax=3500M -p MemorySwapMax=0 $R/build/gpui/hub/backplane-hub-full --home $D/home --port 3816 > $D/hub.log 2>&1 &
for i in $(seq 1 100); do curl -s -m 1 http://127.0.0.1:3816/hello >/dev/null && break; sleep 0.2; done
PID=$(awk '{print $1}' $D/home/hub.lock)
tr '\0' '\n' < /proc/$PID/environ | grep -q "BACKPLANE_AGENTS=$D/bin" || { echo "hub does not run stand-in agents: stop"; kill $PID; exit 1; }
echo '[{"select":15},{"wait":800},{"type":"hello again"},{"send":true},{"wait":3000}]' > $D/script.json
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json DISPLAY=:61 timeout 50 systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 \
  $R/build/gpui/linux/backplane-gpui --url http://127.0.0.1:3816 --token-file $D/home/token --exit-after 30 --script $D/script.json 2> $D/gpui.err
grep GPUI_METRIC $D/gpui.err | grep -E '"usable"|"input"|script_done' | cut -c1-200
kill $PID
