#!/bin/sh
# The management panels on the Android data app (DataMgmt.kt), on the local headless emulator against a hub of our own (scripts/mobile/data-hub.sh)
# with the stand-in parts search (test/kicad_embed/lib/pcbparts_stub.py: no lookup leaves the machine). Screenshots: $OUT/shots/mgmt-*.png.
#   BOM: the tab under the schematic, two-line rows with stock in words, a tap lights the parts in the drawing, a long press edits the MPN (the
#   hub's call, the person's edit: in the saved schematic here, no KiCad open), the panel's Undo; Checks: the sources and the findings by rule.
# usage: scripts/mobile/android-mgmt.sh [APK]   (APK default mobile/android's debug build; HUBBIN default native/host's fast build)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
[ -x "$HOME/.bun/bin/bun" ] && PATH="$HOME/.bun/bin:$PATH"; export PATH
export EMU_PORT=${EMU_PORT:-5594}
E=$ROOT/scripts/android-emu.sh; UI="python3 $ROOT/scripts/android-ui.py"; A="$E adb shell"; PKG=dev.backplane.mobile.gpui
OUT=${OUT:-$BP_TMP/lane-d-phone}; S=$OUT/shots; mkdir -p "$S"
APK=${1:-$ROOT/mobile/android/app/build/outputs/apk/debug/app-debug.apk}
export H=$OUT/hub PORT=${PORT:-3853} HUBBIN=${HUBBIN:-$ROOT/native/host/target/fast/backplane-hub}
FAILS=0
shot() { $E adb exec-out screencap -p > "$S/mgmt-$1.png"; }
xy() { $UI find "$@" 2>/dev/null; }
has() { $UI has "$@" 2>/dev/null; }
tapt() { p=$(xy "$@") || { echo "FAIL: no '$1' on screen"; shot "fail"; FAILS=$((FAILS+1)); return 1; }; $A input tap $p; sleep 1.5; }
longt() { p=$(xy "$@") || { echo "FAIL: no '$1' on screen"; FAILS=$((FAILS+1)); return 1; }; set -- $p; $A input swipe $1 $2 $1 $2 900; sleep 1.5; }
want() { for i in $(seq ${WAIT:-10}); do has "$@" && { echo "ok   $*"; return 0; }; sleep 1; done; echo "FAIL: '$1' not shown"; shot "fail-$(echo "$1" | tr -c 'a-zA-Z0-9' _)"; FAILS=$((FAILS+1)); return 1; }
check() { if [ "$2" = 1 ]; then echo "ok   $1"; else echo "FAIL: $1 ${3:-}"; FAILS=$((FAILS+1)); fi; }

# the stand-in parts search, and the hub of our own with it (the checks run on the project's saved files with this KiCad)
SPORT=$(( PORT + 100 )); python3 "$ROOT/test/kicad_embed/lib/pcbparts_stub.py" $SPORT "$OUT/pcbparts.log" & STUB=$!
trap 'kill $STUB 2>/dev/null; "$ROOT/scripts/mobile/data-hub.sh" stop >/dev/null 2>&1' EXIT
export BACKPLANE_PCBPARTS_URL=http://127.0.0.1:$SPORT/mcp BACKPLANE_PARTS_GAP_MS=300 BACKPLANE_PARTS_LIFE_GAP_S=2 BACKPLANE_PARTS_DK_GAP_S=2
RT=${KICAD_RT:-$HOME/.backplane-gpui-fresh/home/kicad/10.99.0-bp.11}; export BACKPLANE_KICAD_CLI=$RT/bin/kicad-cli
"$ROOT/scripts/mobile/data-hub.sh" start >/dev/null || { echo "FAIL: the hub did not start"; exit 1; }
# the board's own schematic (data-hub's is another design's, which the headless KiCad cannot open for an edit), with MPNs the stand-in knows:
# R1 in stock, C1 out, C2 low
cp "$ROOT/test/files/fixtures/plot/ecc83-pp.kicad_sch" "$H/proj/ecc83.kicad_sch"
python3 - "$H/proj/ecc83.kicad_sch" <<'PY'
import sys
p = sys.argv[1]; s = open(p, encoding="utf-8").read()
for ref, mpn in (("R1", "E2E-FILE"), ("C1", "E2E-LIVE"), ("C2", "E2E-LOW")):
    k = s.index(f'(property "Reference" "{ref}"')
    s = s[:k] + f'(property "MPN" "{mpn}" (at 0 0 0) (effects (font (size 1.27 1.27)) (hide yes))) ' + s[k:]
open(p, "w", encoding="utf-8").write(s)
PY

# the app: installed, paired with this hub
$E start >/dev/null; for i in $(seq 90); do [ "$($E adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ] && break; sleep 2; done
$E adb install -r "$APK" >/dev/null || { echo "FAIL: install"; exit 1; }
# a fresh image's Bluetooth stack crashes and its "keeps stopping" box covers the app: off, and any system box shut
$E adb root >/dev/null 2>&1; sleep 2; $A cmd bluetooth_manager disable >/dev/null 2>&1; $A svc bluetooth disable >/dev/null 2>&1
$A am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null 2>&1; sleep 1
dismiss() { for b in "Close app" "Wait"; do has "$b" exact && tapt "$b" exact; done; $A am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null 2>&1; }
LINK=$("$ROOT/scripts/mobile/data-hub.sh" link); ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "$LINK")
$A pm clear $PKG >/dev/null; $A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 6
if has "Pair with this hub?"; then set -- $($UI bounds "Pair" exact $(( $($UI count "Pair" exact) - 1 ))); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 4; fi
dismiss; has "Not now" exact && tapt "Not now" exact
# the project's threads: unfolded if the list shows it folded
has "Fix the ecc83 board" || tapt "demo" exact
want "Fix the ecc83 board"; tapt "Fix the ecc83 board"; tapt "Viewer" exact; WAIT=30 want "Schematic" exact; tapt "Schematic" exact; sleep 6
for i in 1 2 3; do $E adb logcat -d -s bpview 2>/dev/null | grep -q "schematic: 1|" && break; tapt "Schematic" exact; sleep 5; done

# BOM
want "BOM" exact; tapt "BOM" exact; WAIT=25 want "parts in"; sleep 6; shot 01-bom
WAIT=20 want "low 3"; want "out 0"
$E adb logcat -c
p=$($UI edit 0 2>/dev/null) && { $A input tap $p; sleep 1; $A input text "E2E-FILE"; sleep 2; $A input keyevent KEYCODE_BACK; sleep 1; }   # the filter
tapt "R1" exact; sleep 3; shot 02-bom-lit
LIT=$($E adb logcat -d -s bpview 2>/dev/null | grep -o "lit R1 -> [0-9]*" | tail -1 | grep -o "[0-9]*$")
p=$($UI edit 0 2>/dev/null) && { $A input tap $p; $A input keyevent KEYCODE_MOVE_END; for i in 1 2 3 4 5 6 7 8 9 10 11 12; do $A input keyevent KEYCODE_DEL; done; $A input keyevent KEYCODE_BACK; sleep 2; }
check "a BOM line lights its part in the schematic (R1: ${LIT:-0} pieces)" "$([ "${LIT:-0}" -gt 0 ] && echo 1 || echo 0)"
p=$($UI edit 0 2>/dev/null) && { $A input tap $p; sleep 1; $A input text "R3"; sleep 2; $A input keyevent KEYCODE_BACK; sleep 1; }   # R3 on screen, wherever the list was scrolled
n=$($UI count "R3" exact); set -- $($UI bounds "R3" exact $(( ${n:-1} - 1 ))); [ -n "$1" ] && { $A input swipe $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )) $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )) 900; sleep 1.5; }   # the row's R3, not the filter's want "DNP" exact; shot 03-bom-edit
p=$($UI edit 0 2>/dev/null) && { $A input tap $p; sleep 1; $A input text "PHONE-MPN-1"; sleep 1; $A input keyevent KEYCODE_BACK; sleep 1; }
tapt "Save" exact; WAIT=40 want "Saved in the schematic."; shot 04-bom-saved
check "the phone's edit is in the schematic file as a standard field" "$(grep -q '(property "MPN" "PHONE-MPN-1"' "$H/proj/ecc83.kicad_sch" && echo 1 || echo 0)"
tapt "Undo" exact; WAIT=40 want "Put back."; sleep 1
check "the panel's Undo put the old value back" "$(grep -q '"PHONE-MPN-1"' "$H/proj/ecc83.kicad_sch" && echo 0 || echo 1)"

# Checks: DRC and ERC on the saved files, by rule
tapt "Checks" exact; WAIT=60 want "All" exact; sleep 2; shot 05-checks
echo "android-mgmt: $FAILS failed; shots in $S"
[ "$FAILS" = 0 ]
