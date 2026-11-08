#!/bin/bash
# xprobe.sh DIR: does the archive's client draw on this machine's Xvfb? (CI diagnosis for the arm64 runner: the client rows' screenshots were all black)
# DIR is an unpacked release archive. Prints, per variant, the mean and spread of a root screenshot taken 6 s after the client started. Never fails.
D=$1; rm -rf /tmp/xprobe-home; ICD=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)
echo "== $(uname -m) Xvfb: $(dpkg -s xvfb | grep ^Version)"; dpkg -l | grep -E 'mesa-vulkan|libvulkan1|xvfb|libgl1-mesa-dri' | awk '{print $2, $3}'
"$D/backplane-hub" --home /tmp/xprobe-home --port 3999 --host 127.0.0.1 --no-tailscale --foreground > /tmp/xprobe-hub.log 2>&1 & HUB=$!; sleep 3
try() { name=$1; shift; N=88; rm -f /tmp/.X11-unix/X$N
  Xvfb :$N -screen 0 1440x900x24 -nolisten tcp "$@" > /tmp/xprobe-xvfb.log 2>&1 & X=$!; sleep 2
  DISPLAY=:$N xdpyinfo -queryExtensions 2>/dev/null | grep -iE 'dri3|present|glx|shm|xtest' | tr '\n' ' '; echo
  DISPLAY=:$N VK_ICD_FILENAMES=$ICD LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$D/assets LD_LIBRARY_PATH=$D "$D/backplane-gpui" --url http://127.0.0.1:3999 --token-file /tmp/xprobe-home/token --exit-after 8000 > /tmp/xprobe-client.log 2>&1 & C=$!; sleep 6
  DISPLAY=:$N import -window root /tmp/xprobe-$name.png 2>&1 | head -2
  python3 -I -c "
from PIL import Image, ImageStat
s=ImageStat.Stat(Image.open('/tmp/xprobe-$name.png').convert('L')); print('$name', 'mean', round(s.mean[0],2), 'stddev', round(s.stddev[0],2))"
  grep -iE 'gpu|vulkan|error|panic' /tmp/xprobe-client.log | head -5 | cut -c1-240
  kill $C $X 2>/dev/null; wait $C $X 2>/dev/null; }
try default
try glx +extension GLX +render -noreset
try dri3 +extension DRI3
kill $HUB 2>/dev/null; wait $HUB 2>/dev/null
exit 0
