#!/bin/sh
# The iOS /give journey (test_give in test/e2e/ios/UITests): a rig hub (give-rig.sh: stand-in agents, --no-tailscale) on the tailnet address, the app built on the Mac, driven in the Simulator
# under the Backplane lock mac-heavy (taken by whoever starts this). Shots land in $OUT (default $BP_TMP/igive/shots/ios).    env: M (the ssh wrapper), SIM_UDID, OUT
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
MAIN=${ROOT%%/.backplane/worktrees/*}; M=${M:-$MAIN/build/gpui/mac-ssh}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; OUT=${OUT:-$BP_TMP/igive/shots/ios}
IP=$(tailscale ip -4 | head -1); D=bp-igive; R=$BP_TMP/give-ios
mkdir -p "$OUT"
"$ROOT/scripts/mobile/give-rig.sh" stop ios; "$ROOT/scripts/mobile/give-rig.sh" start ios "$IP" 3811 || exit 1
RSYNC_RSH=$M rsync -a --delete --exclude build "$ROOT/mobile/ios/" hs-mac-mini:$D/ios/ || exit 1
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$D/e2e/ || exit 1
RSYNC_RSH=$M rsync -a "$R/home/token" hs-mac-mini:$D/token || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/'$D'/ios || exit 1
lockf -k /tmp/bp-mac-heavy.lockf sh -c "
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build 2>&1 | grep -E \"error:\" | head -30
cd ~/'$D'/e2e && rm -rf ~/'$D'/out && SIM_UDID='$U' OUT=\$HOME/'$D'/out HUB_HOST='$IP' HUB_PORT=3811 HUB_TOKEN_FILE=\$HOME/'$D'/token \
  APP=\$HOME/'$D'/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/'$D'/e2e-dd sh ./run.sh give
xcrun simctl shutdown '$U'"
'
RSYNC_RSH=$M rsync -a --exclude "*.xcresult" --exclude "*.mp4" hs-mac-mini:$D/out/ "$OUT/"
"$ROOT/scripts/mobile/give-rig.sh" stop ios
python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); print(j["status"], [(s["do"][:60], s["ok"]) for s in j["metrics"]["steps"]])' "$OUT/give.json" || tail -20 "$OUT/give.xcodebuild.log"
