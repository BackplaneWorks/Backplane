#!/bin/sh
# Real X input (XTest) on the board canvas, shots in between: wheel zoom, drag pan, double-click fit, the keys. Run by desktop-plot-run.sh (GPUI_DRIVE).
X=$(dirname "$0")/../cef/xinput.py; I() { python3 "$X" "$@"; }; shot() { import -window root "$SHOTS/$NAME-$1.png"; }
CX=1108; CY=500
shot c0
for i in 1 2 3 4; do I wheel $CX $CY 1; done; sleep 1; shot c1-wheel-zoom
I drag 1000 450 1180 560; sleep 1; shot c2-drag-pan
I dblclick 1250 800; sleep 1.5; shot c3-dblclick-fit
I wheel $CX $CY 1; I wheel $CX $CY 1; I wheel $CX $CY 1; sleep 0.5
I click 1250 820; sleep 0.5          # focus the canvas
for i in 1 2 3; do I key Right; done; I key plus; I key plus; sleep 1; shot c4-keys-pan-zoom
I key 0; sleep 1; shot c5-key-fit
