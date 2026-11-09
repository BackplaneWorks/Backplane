#!/bin/sh
# The iOS "every send interrupts" acceptance (test_interrupt_everywhere in test/e2e/ios/UITests) on a two-hub rig: hub A owns "Far work", hub B (the app's)
# "Local work" and "Helper", linked as one owner's machines, stand-in claude streaming; B reaches A only through relays that can be cut, the app reaches B only
# through a relay on this machine's tailnet address. The rig is $RIGSRC/rig.sh (+ ctl.py, the test's hands: /cutA /healA /cutB /healB /send /helper /now).
# Then the app's unit tests. Results, screenshots and the stand-in logs in $OUT. Run under the Backplane lock mac-heavy.
#   scripts/mobile/ios-interrupt-two.sh        env: M (the ssh wrapper), SIM_UDID, OUT, RIGSRC, D (the Mac scratch dir)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
MAIN=${MAIN:-$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; OUT=${OUT:-$BP_TMP/interrupt-ios2}
RIGSRC=${RIGSRC:-$ROOT/test/e2e/interrupt/ios}; R=$BP_TMP/ios2r; D=${D:-bp-interrupt-ios2}; IP=$(tailscale ip -4 | head -1)
mkdir -p "$OUT"
"$RIGSRC/rig.sh" down; sleep 1; "$RIGSRC/rig.sh" up || exit 1
timeout 90 "$RIGSRC/rig.sh" pair | grep -E "state" || exit 1
sleep 6; "$RIGSRC/rig.sh" hub B
i=0; until ls "$R"/homeB/far/*.jsonl >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 60 ] && { echo "B never mirrored A"; exit 1; }; sleep 1; done
LINK=$(python3 -c 'import json,sys; print([json.loads(l)["id"] for l in open(sys.argv[1]) if "PeerSet" in l][-1])' "$R/homeB/events.jsonl"); FAR="$LINK~ta"; echo "far thread $FAR"
$M hs-mac-mini "rm -rf ~/$D && mkdir -p ~/$D/src/mobile ~/$D/src/scripts ~/$D/src/test" || exit 1
for p in mobile/ios scripts/mobile test/e2e/ios test/mobile test/fixtures; do RSYNC_RSH=$M rsync -a --exclude build "$ROOT/$p/" hs-mac-mini:$D/src/$p/ || exit 1; done
RSYNC_RSH=$M rsync -a "$R/homeB/token" hs-mac-mini:$D/token || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
defaults write com.apple.iphonesimulator ConnectHardwareKeyboard -bool false
cd ~/'$D'/src/mobile/ios || exit 1
lockf -k /tmp/bp-mac-heavy.lockf sh -c "
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath ~/'$D'/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build 2>&1 | grep -E \"error:\" | head -30
xcrun simctl boot '$U' 2>/dev/null; xcrun simctl uninstall '$U' dev.backplane.gpui 2>/dev/null
cd ~/'$D'/src/test/e2e/ios && SIM_UDID='$U' OUT=\$HOME/'$D'/out HUB_HOST='$IP' HUB_PORT=3993 HUB_TOKEN_FILE=\$HOME/'$D'/token HUB_CTL=http://'$IP':3998 LOCAL_THREAD=tb FAR_THREAD='"'$FAR'"' \
  APP=\$HOME/'$D'/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/'$D'/e2e-dd sh ./run.sh interrupt-everywhere
xcrun simctl uninstall '$U' dev.backplane.gpui 2>/dev/null; xcrun simctl shutdown '$U'
cd ~/'$D'/src && sh scripts/mobile/ios-unit-tests.sh > ~/'$D'/out/unit-tests.log 2>&1; echo unit-tests exit \$? >> ~/'$D'/out/unit-tests.log"
'
RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ "$OUT/"
cp "$R/standin-A.log" "$R/standin-B.log" "$R/ctl.log" "$R/mcp.urls" "$OUT/" 2>/dev/null
python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); print(j["status"], j["notes"][:-1], {k:v for k,v in j["metrics"].items() if k.endswith("_ms") or k.startswith("far")}); [print(" ", s["ok"], s["result_ms"], s["do"]) for s in j["metrics"]["steps"]]' "$OUT/interrupt-everywhere.json" || tail -20 "$OUT/interrupt-everywhere.xcodebuild.log"
tail -15 "$OUT/unit-tests.log"
