#!/bin/sh
# The drawing pad on the Android data app, end to end: the local emulator (scripts/android-emu.sh start) against scripts/mobile/data-hub.sh's hub.
# Opens a thread, draws a stroke and a box, gives the box a typed width, attaches the drawing and sends it. Screenshots: build/mobile/shots/pad-*.png.
# usage: test/pad/pad_android.sh      (needs the debug APK installed and the hub started: HUBBIN=<a backplane-hub> scripts/mobile/data-hub.sh start)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/android-emu.sh; UI="python3 $ROOT/scripts/android-ui.py"; S=$ROOT/build/mobile/shots; mkdir -p $S
A="$E adb shell"; PKG=dev.backplane.mobile.gpui
shot() { $E adb exec-out screencap -p > $S/pad-$1.png; }
xy() { $UI find "$@" 2>/dev/null; }
tapt() { p=$(xy "$@") || { echo "FAIL: no '$1' on screen"; shot "fail"; exit 1; }; $A input tap $p; sleep 1.2; }
has() { $UI has "$@" 2>/dev/null; }
want() { for i in 1 2 3 4 5 6; do has "$@" && { echo "ok   $*"; return 0; }; sleep 1; done; echo "FAIL: '$1' not shown"; shot "fail"; exit 1; }
gone() { sleep 1; has "$@" && { echo "FAIL: '$1' still shown"; exit 1; }; echo "ok   gone: $*"; }
# the paper's corner and size on screen; a paper point (960 x 600) to a screen point
paper() { set -- $($UI bounds "Drawing paper") || { echo "FAIL: no paper"; exit 1; }; L=$1; T=$2; R=$3; B=$4; }
at() { echo $((L + ($R - L) * $1 / 960)) $((T + ($B - T) * $2 / 600)); }
swipe() { $A input swipe $(at $1 $2) $(at $3 $4) ${5:-500}; sleep 1; }
# what a pixel of a screenshot is, as "r g b"
px() { python3 -c "from PIL import Image; import sys; print(*Image.open(sys.argv[1]).convert('RGB').getpixel((int(sys.argv[2]), int(sys.argv[3]))))" $S/pad-$1.png $2 $3; }
white() { [ "$(px "$@")" = "255 255 255" ]; }

LINK=$($ROOT/scripts/mobile/data-hub.sh link); ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "$LINK")
$A pm clear $PKG >/dev/null; $A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 5
want "Fix the ecc83 board"; tapt "Fix the ecc83 board"; want "Send" exact
tapt "Draw" exact; want "Drawing paper"; want "Drag to draw"; shot 01-open
paper; echo "ok   paper at $L $T $R $B"
# a stroke with the pen, in the first colour
swipe 100 100 400 100; shot 02-stroke
set -- $(at 250 100); white 02-stroke $1 $2 && { echo "FAIL: the stroke left no ink"; exit 1; }; echo "ok   the pen draws"
# a box: the Box tool, dragged out; then a tap on its right side asks for its width
tapt "Box" exact; want "tap a side"
swipe 500 250 800 500 600; shot 03-box
set -- $(at 650 375); white 03-box $1 $2 && { echo "FAIL: no box"; exit 1; }; echo "ok   a box is drawn"
want "Delete" exact   # the new box is picked
$A input tap $(at 800 375); sleep 1.5; want "Length, e.g. 40 mm"; shot 04-length
p=$($UI edit 0) && $A input tap $p; for i in 1 2 3 4 5 6; do $A input keyevent KEYCODE_DEL; done; $A input text "40%smm"; sleep 1; tapt "Set" exact
gone "Length, e.g. 40 mm"; shot 05-length-set
# the width is now 40 units (160 paper px): the right side moved in from 800 to 660, the old right edge is paper again
set -- $(at 760 375); white 05-length-set $1 $2 || { echo "FAIL: the typed width did not resize the box"; exit 1; }; echo "ok   a typed width resizes the box"
# undo takes it back, and again
tapt "Undo" exact; shot 06-undo; set -- $(at 760 375); white 06-undo $1 $2 && { echo "FAIL: undo did not bring the width back"; exit 1; }; echo "ok   undo"
# the eraser cuts the stroke
tapt "Eraser" exact; swipe 250 50 250 150; shot 07-erased
set -- $(at 250 100); white 07-erased $1 $2 || { echo "FAIL: the eraser left the ink"; exit 1; }
set -- $(at 130 100); white 07-erased $1 $2 && { echo "FAIL: the eraser took the whole stroke"; exit 1; }; echo "ok   the eraser cuts a stroke"
# attach: the pad closes and the picture waits in the composer; send it
tapt "Attach" exact; gone "Drawing paper"; shot 08-attached
tapt "Send" exact; sleep 4; shot 09-sent
python3 $ROOT/scripts/android-ui.py dump > /dev/null 2>&1
grep -q '"name":"photo-1.jpg"' $ROOT/build/mobile/hub/home/events.jsonl && echo "ok   the hub has the drawing" || { echo "FAIL: the hub got no picture"; exit 1; }
echo "pad: all ok"
