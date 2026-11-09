#!/bin/sh
# The iOS interrupt/outbox journey (test_interrupt_outbox in test/e2e/ios/UITests): the term rig's hub and relay (ios-term-rig.sh, a hub built from this tree,
# stand-in claude, --no-tailscale), the app built on the Mac, the journey in the Simulator. Results and screenshots in $OUT (default $BP_TMP/interrupt-ios),
# the stand-in's log beside them (NOW lines: a message put into a running turn). Run under the Backplane lock mac-heavy.
#   scripts/mobile/ios-interrupt-journey.sh        env: M (the ssh wrapper), SIM_UDID, OUT
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; OUT=${OUT:-$BP_TMP/interrupt-ios}
IP=$(tailscale ip -4 | head -1); D=bp-interrupt-ios; R=$BP_TMP/iterm-rig
mkdir -p "$OUT"
"$ROOT/scripts/mobile/ios-term-rig.sh" stop; sleep 2; "$ROOT/scripts/mobile/ios-term-rig.sh" start || exit 1; sleep 7
S=$(cd "$ROOT/test/e2e/ios" && ~/.bun/bin/bun interrupt-seed.ts "$IP" 3801 "$R/home/token" "$R/proj") || exit 1
THREAD=$(echo "$S" | python3 -c 'import json,sys; print(json.load(sys.stdin)["thread"])'); echo "thread $THREAD"
RSYNC_RSH=$M rsync -a --delete --exclude build "$ROOT/mobile/ios/" hs-mac-mini:$D/ios/ || exit 1
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$D/e2e/ || exit 1
RSYNC_RSH=$M rsync -a "$R/home/token" hs-mac-mini:$D/token || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
defaults write com.apple.iphonesimulator ConnectHardwareKeyboard -bool false
cd ~/'$D'/ios || exit 1
lockf -k /tmp/bp-mac-heavy.lockf sh -c "
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build 2>&1 | grep -E \"error:\" | head -30
xcrun simctl boot '$U' 2>/dev/null; xcrun simctl uninstall '$U' dev.backplane.gpui 2>/dev/null
cd ~/'$D'/e2e && rm -rf ~/'$D'/out && SIM_UDID='$U' OUT=\$HOME/'$D'/out HUB_HOST='$IP' HUB_PORT=3802 HUB_TOKEN_FILE=\$HOME/'$D'/token HUB_CTL=http://'$IP':3803 INTERRUPT_THREAD='$THREAD' \
  APP=\$HOME/'$D'/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/'$D'/e2e-dd sh ./run.sh interrupt-outbox
xcrun simctl shutdown '$U'"
'
RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ "$OUT/"
cp "$R/standin.log" "$OUT/standin.log" 2>/dev/null
python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); print(j["status"], j["notes"][:-1]); [print(" ", s["ok"], s["do"]) for s in j["metrics"]["steps"]]' "$OUT/interrupt-outbox.json" || tail -20 "$OUT/interrupt-outbox.xcodebuild.log"
"$ROOT/scripts/mobile/ios-term-rig.sh" stop
