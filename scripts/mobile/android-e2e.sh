#!/bin/sh
# End to end for the Android data app on the local emulator against scripts/mobile/data-hub.sh's hub (stand-in agent). Screenshots: build/mobile/shots/e2e-*.png.
# Needs the app installed (mobile/android debug APK) and the emulator and hub running. Each step asserts by reading the UI tree (uiautomator); exit 1 at the first miss.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/gpui/android-emu.sh; UI="python3 $ROOT/scripts/gpui/android-ui.py"; S=$ROOT/build/mobile/shots; mkdir -p $S
A="$E adb shell"; PKG=dev.backplane.mobile.gpui
shot() { $E adb exec-out screencap -p > $S/e2e-$1.png; }
xy() { $UI find "$@" 2>/dev/null; }
tapt() { p=$(xy "$@") || { echo "FAIL: no '$1' on screen"; shot "fail-$(echo $1 | tr -c 'a-zA-Z0-9' _)"; exit 1; }; $A input tap $p; sleep 1.2; }
longt() { p=$(xy "$@") || { echo "FAIL: no '$1' on screen"; exit 1; }; set -- $p; $A input swipe $1 $2 $1 $2 900; sleep 1.2; }
tapf() { p=$($UI edit ${1:-0} 2>/dev/null) || { echo "FAIL: no text field"; exit 1; }; $A input tap $p; sleep 1; }
has() { $UI has "$@" 2>/dev/null; }
want() { for i in 1 2 3 4 5 6; do has "$@" && { echo "ok   $*"; return 0; }; sleep 1; done; echo "FAIL: '$1' not shown"; shot "fail-$(echo $1 | tr -c 'a-zA-Z0-9' _)"; exit 1; }
gone() { sleep 1; has "$@" && { echo "FAIL: '$1' still shown"; exit 1; }; echo "ok   gone: $*"; }
typ() { $A input text "$1"; sleep 1; }
gohome() { for i in 1 2 3; do has "Add project" exact && return 0; $A input keyevent KEYCODE_BACK; sleep 1.2; done; has "Add project" exact || { echo "FAIL: not home"; exit 1; }; }
back() { $A input keyevent KEYCODE_BACK; sleep 1.2; }
$E adb root >/dev/null 2>&1; sleep 3   # the emulator image allows it: sendevent (a two-finger pinch) needs it
LINK=$($ROOT/scripts/mobile/data-hub.sh link); ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "$LINK")
$A pm clear $PKG >/dev/null; $A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 5
want "Fix the ecc83 board"; want "ada"; want "lab"; shot 01-home
if [ "${ONLY:-}" != viewer ]; then   # ONLY=viewer runs just the viewer steps (after pairing)
# search
tapt "Search" exact; tapf; typ bom; want "BOM review"; gone "Fix the ecc83 board"; shot 02-search; tapt "Search" exact
# thread menu: pin, rename
longt "BOM review"; want "Pin"; shot 03-rowmenu; tapt "Pin" exact
longt "BOM review"; tapt "Rename" exact; $A input keyevent KEYCODE_MOVE_END; for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14; do $A input keyevent KEYCODE_DEL; done; typ "Parts%sreview"; shot 04-rename; tapt "Rename" exact
want "Parts review"
# open a thread: model picker, settle from the header menu
tapt "Fix the ecc83 board"; want "claude / default"; tapt "claude / default"; want "fable"; shot 05-models; tapt "opus"; want "claude / opus"
tapt "More" exact; want "Settle"; shot 06-threadmenu; tapt "Settle" exact
# mention popup
tapf; typ "@a"; want "@ada"; shot 07-mention; tapt "@ada"
# ask card: approve
$A input keyevent KEYCODE_CTRL_LEFT >/dev/null 2>&1
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do $A input keyevent KEYCODE_DEL; done
typ "approve:%sgo"; tapt "Send" exact; want "Allow" ; shot 08-ask; tapt "Allow" exact; want "answered"; shot 09-answered
gohome
# bots and rooms: a bot's thread, a room post
tapt "ada"; want "STATE"; gohome
tapt "lab"; typ_room=1; tapf; typ "hello%sroom"; tapt "Post" exact; want "hello room"; shot 10-room; gohome
# add project: folder browser
tapt "Add project"; want "Use this folder"; shot 11-folders; tapt "Use this folder"; sleep 2; want "New thread"; shot 12-newthread; gohome
# attach a picture: the system picker, then send it
$E adb push $ROOT/test/mobile/pic.png /sdcard/Download/bp-test.png >/dev/null; $A am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///sdcard/Download/bp-test.png >/dev/null
tapt "Fix the ecc83 board"; tapt "Attach" exact; sleep 2; shot 12b-picker
tapt "Photo taken"; sleep 3; want "Send"; shot 12c-attached; tapt "Send" exact; sleep 4; shot 12d-picture-sent
gohome
# alerts: a turn that ends while the app is in the background raises a notification
tapt "BOM review"; tapf; typ "notify%scheck"; tapt "Send" exact; $A input keyevent KEYCODE_HOME; sleep 7
$A dumpsys notification --noredact | grep -q "data-alerts" && echo "ok   notification posted (data-alerts)" || { echo "FAIL: no notification"; exit 1; }
$E adb exec-out screencap -p > $S/e2e-14-home.png
$A cmd statusbar expand-notifications; sleep 2; shot 14-notification; $A cmd statusbar collapse; $A "am start -n $PKG/dev.backplane.mobile.DataActivity" >/dev/null; sleep 2; gohome
# improvements
tapt "Settings" exact; tapt "Improvements" exact; tapf; typ "make%sit%sfaster"; shot 13-improve; tapt "Send to an agent thread" exact; want "Thread started"; shot 14b-improve-sent; gohome; want "Improvement idea"; shot 15-final
fi
# viewer: Board (layers, tap to inspect), Schematic, 3D, Mech, Files (image, PDF, markdown, open a board), Browser
tapt "Fix the ecc83 board"; tapt "Viewer" exact; want "Board"; sleep 6; want "Layers" exact; shot 20-board
tapt "Layers" exact; want "B.Cu"; shot 21-layers; tapt "B.Cu" exact; sleep 1; shot 22-layer-focus; tapt "B.Cu" exact; tapt "Layers" exact
for xy in "506 1682" "184 1206" "184 1338" "870 1200" "840 1090"; do $A input tap $xy; sleep 1; has "PICKED" && break; done
has "PICKED" && { echo "ok   tap to inspect shows what was picked"; shot 23-picked; } || { echo "FAIL: a tap on a pad picked nothing"; exit 1; }
# pan: a one-finger drag moves the board (the two screenshots differ)
shot 23a-before-pan; $A input swipe 700 1300 300 1000 400; sleep 1; shot 23b-after-pan
python3 - "$S/e2e-23a-before-pan.png" "$S/e2e-23b-after-pan.png" <<'PY' || { echo "FAIL: a drag did not move the board"; exit 1; }
import sys
from PIL import Image, ImageChops
a, b = (Image.open(p).convert("L") for p in sys.argv[1:3])
d = ImageChops.difference(a, b); n = sum(1 for v in d.getdata() if v > 40)
print("ok   a drag moves the board (%d pixels changed)" % n); sys.exit(0 if n > 20000 else 1)
PY
# pinch: two fingers spread (sendevent on the emulator's touch screen; the device is the one whose events change the picture) zoom the board in, and together out
shot 23c-before-pinch; DEV=""
for d in 2 12 11 10 9 8 7 6 5 4 3 1; do $ROOT/scripts/mobile/android-pinch.sh /dev/input/event$d out >/dev/null 2>&1; sleep 1; shot 23d-after-pinch
  if python3 -c "
import sys
from PIL import Image, ImageChops
a, b = (Image.open('$S/e2e-' + n + '.png').convert('L') for n in ('23c-before-pinch', '23d-after-pinch'))
sys.exit(0 if sum(1 for v in ImageChops.difference(a, b).getdata() if v > 40) > 40000 else 1)" 2>/dev/null; then DEV=/dev/input/event$d; break; fi; done
[ -n "$DEV" ] && echo "ok   a two-finger pinch zooms the board ($DEV)" || { echo "FAIL: no touch device took a pinch"; exit 1; }
$ROOT/scripts/mobile/android-pinch.sh $DEV in >/dev/null 2>&1; sleep 1; shot 23e-pinch-in
tapt "Schematic" exact; sleep 6; want "Layers" exact; shot 24-schematic
tapt "3D" exact; sleep 8; shot 25-3d; gone "Unable"
tapt "Mech" exact; want "bracket.step"; sleep 8; shot 26-mech; gone "Unable"
tapt "Files" exact; want "docs"; shot 27-files; tapt "docs" exact; want "picture.png"; tapt "picture.png"; sleep 2; want "‹ picture.png"; shot 28-image; tapt "‹ picture.png"
tapt "note.pdf"; sleep 2; want "page 1 of 1"; shot 29-pdf; tapt "‹ note.pdf"
tapt "notes.md"; sleep 2; want "tube socket moves"; shot 30-markdown; tapt "‹ notes.md"
tapt ".." exact; want "ecc83.kicad_pcb"; tapt "ecc83.kicad_pcb"; sleep 5; want "Layers" exact; shot 31-board-from-files
tapt "Split" exact; sleep 9; shot 34-split; $UI has "Schematic" >/dev/null; want "SCHEMATIC"; want "BOARD"; want "3D"
echo "diff-marker-e2e" >> $ROOT/build/mobile/hub/proj/README.md
tapt "Diff" exact; sleep 3; want "CHANGES"; if [ "${ONLY:-}" = viewer ]; then want "No changes"; else want "diff-marker-e2e"; fi; shot 35-diff
$A input swipe 900 320 100 320 250; sleep 1; tapt "Review" exact; sleep 2; want "Checks" exact; tapt "Run checks" exact; sleep 8; shot 36-checks; tapt "BOM" exact; sleep 6; shot 37-bom
tapt "Comments" exact; sleep 2; tapf; typ "check%sthe%sdecoupling"; tapt "Add" exact; sleep 3; want "check the decoupling"; shot 38-comment; tapt "resolve"; sleep 2; want "resolved"; shot 39-resolved
if [ "${LIVE:-}" = 1 ]; then
  # a real page in the helper's Chrome: take over, open it by address, tap its button, and the page's own server hears the click
  rm -f /tmp/live-clicks.txt; python3 $ROOT/test/mobile/live_page.py 3899 /tmp/live-clicks.txt & LP=$!
  $A input swipe 900 320 100 320 250; sleep 1; tapt "Browser" exact; sleep 2; tapt "Take over"; want "Hand back"
  tapf; $A input text "127.0.0.1:3899"; sleep 1; tapt "Go" exact; sleep 12; shot 32l-live-page; shot 32l2-live-page
  $A input keyevent KEYCODE_BACK; sleep 2; shot 33l0-live-no-keyboard
  $A input tap 540 1395; sleep 4; shot 33l-live-after-click
  kill $LP 2>/dev/null
  [ -s /tmp/live-clicks.txt ] && echo "ok   a tap on the page's button reached the page (the server heard $(wc -l < /tmp/live-clicks.txt) click)" || { echo "FAIL: the page heard no click"; exit 1; }
  echo "e2e: all steps passed"; exit 0
fi
$A input swipe 900 320 100 320 250; sleep 1; tapt "Browser" exact; sleep 3; want "Take over"; shot 32-browser; tapt "Take over"; want "Hand back"; $A input tap 540 700; sleep 2; gone "take over the browser first"; shot 33-browser-driving
echo "e2e: all steps passed"
