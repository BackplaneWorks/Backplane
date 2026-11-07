#!/bin/sh
# live_ios.sh CLIENT_BIN HUB_BIN PYTHON : the Order card of a live buy thread on the iOS data app, on a simulator of this lane's own on the Mac
# (MAC, default hs-mac-mini). live_e2e.sh makes the buy thread in the window and stops at the Order card (LIVE_NO_PAY=1); its hub, the order
# server and the Stripe double are forwarded onto the Mac's loopback (ssh -R), so the simulator and its Safari reach them at 127.0.0.1.
# The order-card journey (test/e2e/ios) opens the buy thread, taps Buy now and waits for Safari, whose test checkout pays the double.
# Screenshots in $SHOTS. Never the real order server or a real payment. Run it under the mac-heavy lock.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); MAC=${MAC:-hs-mac-mini}; R=backplane-ios-commerce
OUT=${OUT:-$HOME/.cache/bp-tmp/order-live}; SHOTS=${SHOTS:-$HOME/.cache/commerce-live/shots/ios}; mkdir -p "$SHOTS"; rm -f "$SHOTS"/*
fails=0; ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
rm -f "$OUT/rig.json"; LIVE_KEEP=1 LIVE_NO_PAY=1 sh "$HERE/live_e2e.sh" "$1" "$2" "$3" > "$HOME/.cache/commerce-live/rig-ios.log" 2>&1 & RIG=$!
for i in $(seq 1 300); do [ -f "$OUT/rig.json" ] && break; kill -0 $RIG 2>/dev/null || break; sleep 1; done
[ -f "$OUT/rig.json" ] || { echo "FAIL the rig did not come up"; cat "$HOME/.cache/commerce-live/rig-ios.log"; exit 1; }
j() { python3 -I -c "import json;print(json.load(open('$OUT/rig.json'))['$1'])"; }
HUB=$(j hub); FAB=$(j fab); STRIPE=$(j stripe); BUY=$(j buy); paid() { grep -c ':paid"' "$OUT/home/events.jsonl"; }
ssh -o BatchMode=yes -o ExitOnForwardFailure=yes -N -R $HUB:127.0.0.1:$HUB -R $FAB:127.0.0.1:$FAB -R $STRIPE:127.0.0.1:$STRIPE $MAC & TUN=$!
sleep 3; kill -0 $TUN 2>/dev/null && ok "the rig's ports are forwarded to the Mac" || bad "the ssh forward"
ssh -o BatchMode=yes $MAC "mkdir -p ~/$R/out"; rsync -a --delete --exclude build "$ROOT/mobile/ios/" $MAC:$R/ios/; rsync -a --delete "$ROOT/test/e2e/ios/" $MAC:$R/e2e/
scp -q "$OUT/home/token" $MAC:$R/token
before=$(paid)
ssh -o BatchMode=yes $MAC "R=$R HUB=$HUB BUY=$BUY sh -s" <<'MAC'
set -u; export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$R
curl -s -o /dev/null -w "hub from the Mac: %{http_code}\n" http://127.0.0.1:$HUB/
( cd ios && xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination "generic/platform=iOS Simulator" \
    -derivedDataPath build/dd SWIFT_ACTIVE_COMPILATION_CONDITIONS="DEBUG GPUI_NATIVE DATA_APP" build > ../app-build.log 2>&1 ) || { tail -20 app-build.log; echo "app build failed"; exit 1; }
SIM=$(xcrun simctl list devices | awk -F'[()]' '/commerce-e2e/ {print $2; exit}')
[ -n "$SIM" ] || SIM=$(xcrun simctl create commerce-e2e "$(xcrun simctl list devicetypes | awk -F'[()]' '/iPhone 1[5-7] / {print $2; exit}')")
echo "simulator $SIM"
SIM_UDID=$SIM OUT=$HOME/$R/out HUB_HOST=127.0.0.1 HUB_PORT=$HUB HUB_TOKEN_FILE=$HOME/$R/token BUY_THREAD=$BUY \
  APP=$HOME/$R/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=$HOME/$R/e2e-dd sh e2e/run.sh order-card
cat out/order-card.json 2>/dev/null | head -c 3000; echo; cat out/order-card.assert.txt 2>/dev/null
xcrun simctl shutdown "$SIM" >/dev/null 2>&1
MAC
scp -q "$MAC:$R/out/order-card-*.png" "$SHOTS/" 2>/dev/null; ls "$SHOTS" | sed 's/^/shot /'
for i in $(seq 1 30); do [ "$(paid)" -gt "$before" ] && break; sleep 2; done
[ "$(paid)" -gt "$before" ] && ok "Safari's test payment came back to the hub as Paid" || bad "the payment came back as Paid"
kill $TUN 2>/dev/null; touch "$OUT/stop"; wait $RIG 2>/dev/null
grep -E '^(PASS|FAIL)' "$HOME/.cache/commerce-live/rig-ios.log" | sed 's/^/window: /'
echo "$fails failed"; exit $fails
