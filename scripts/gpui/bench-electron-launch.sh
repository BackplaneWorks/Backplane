#!/bin/sh
R=/home/h/Documents/i2cjak/backplane-gpui
exec systemd-run --user --scope --quiet -p MemoryMax=3700M -p MemorySwapMax=0 env DISPLAY=:62 WAYLAND_DISPLAY= HOME=$R/build/gpui/perf-run/cmp/fakehome-el BACKPLANE_HOME=$R/build/gpui/perf-run/cmp/home BACKPLANE_SERVICE_VERSION=0.14.0 BACKPLANE_NO_UPDATE=1 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json /home/h/.local/share/backplane/desktop/runtime/backplane-desktop --remote-debugging-port=9231 --ozone-platform=x11 "$@"
