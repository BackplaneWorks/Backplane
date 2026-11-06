#!/bin/sh
# 3D page measurements on Boat (Xvfb, llvmpipe = software Vulkan: the atlas is process memory there, so RSS shows what the atlas keeps).
# Synthetic spheres through the real frame path (see viewers/demo.rs). After scripts/boat-build.sh. Output build/gpui-solid/.
set -u
[ "$(id -un)" = user ] || { echo "boat-solid.sh: not a Boat sandbox (user $(id -un)); refusing" >&2; exit 3; }
V=build/gpui-solid; mkdir -p $V; : > $V/results.txt
BIN=$PWD/build/gpui-target/release/backplane-gpui; export BACKPLANE_GPUI_ASSETS=$PWD/build/gpui-artifacts/assets
say() { echo "$*" | tee -a $V/results.txt; }
need() { command -v "$1" >/dev/null 2>&1; }
if ! need Xvfb || ! need i3; then (sudo -n apt-get update -qq && sudo -n apt-get install -y -qq xvfb i3 x11-xserver-utils mesa-vulkan-drivers) > $V/apt.log 2>&1 || say "apt failed"; fi
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > $V/i3.conf
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
Xvfb :61 -screen 0 1440x900x24 -nolisten tcp > $V/xvfb.log 2>&1 & XP=$!
sleep 2; DISPLAY=:61 i3 -c $V/i3.conf > $V/i3.log 2>&1 & IP=$!
printf 'Xft.dpi: 96\n' | DISPLAY=:61 xrdb -merge; sleep 1
run() { # name spec [env]
  n=$1; spec=$2; shift 2
  env DISPLAY=:61 "$@" timeout 170 $BIN --url http://127.0.0.1:1 --token-file /dev/null --solid-demo $spec --exit-after 150 > $V/$n.log 2>&1
  say "$n ($spec $*): $(grep -m1 '"event":"solid_demo"' $V/$n.log | cut -c12-700)"
}
grep -m1 '"event":"gpu"' /dev/null
run drop-100k "100000,400"
run drop-800k "800000,60"
kill $IP $XP 2>/dev/null; wait $IP $XP 2>/dev/null
nproc | sed 's/^/cores: /' | tee -a $V/results.txt
exit 0
