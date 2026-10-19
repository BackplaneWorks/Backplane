#!/bin/sh
set -eu
. scripts/bendok.sh
export BACKPLANE_LOW_MEM=1 BUN_JSC_forceRAMSize=4294967296
mkdir -p build/fallback-ui
printf 'Compiler availability: '
for cc in clang clang-19 clang-20 clang-21 gcc cc; do command -v "$cc" || true; done
printf 'X11 header: '
[ ! -f /usr/include/X11/Xlib.h ] || echo present
bend_check test/native/ui_snap.bend
bend test/native/ui_snap.bend -o build/fallback-ui/all.c
