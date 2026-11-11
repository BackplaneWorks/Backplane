#!/bin/sh
# Swarms on the iOS app (docs/SWARM.md) on the Mac mini, in a folder and a simulator of its own, against test/swarm/rig.py's hub on this machine's tailnet
# address: builds the app, runs the iOS unit tests there (scripts/mobile/ios-unit-tests.sh) and the journey `swarm` (Journeys.test_swarm), which takes the screenshots.
# usage: test/swarm/ios_swarm.sh HUB_BIN        Start with work_run {lock: "mac-heavy"}. Results and shots: build/mobile/ios-swarm.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}
HUBBIN=$1
D=${D:-bp-swarm-ios}; OUT=$ROOT/build/mobile/ios-swarm; rm -rf "$OUT"; mkdir -p "$OUT"
. "$ROOT/scripts/lib-bptmp.sh"; R=$BP_TMP/swarm-ios; RIG="python3 -I $ROOT/test/swarm/rig.py"
# the app, built in the Mac's own folder
M=$M D=$D "$ROOT/scripts/mobile/ios-check.sh" | tail -8 | tee "$OUT/build.txt"
$M hs-mac-mini "test -d ~/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app" || { echo "the app did not build"; exit 1; }
# the unit tests, on the Mac (the catalog gates first)
$M hs-mac-mini "rm -rf ~/$D/repo && mkdir -p ~/$D/repo/test" || exit 1
RSYNC_RSH=$M rsync -a --exclude build $ROOT/mobile $ROOT/scripts $ROOT/i18n hs-mac-mini:$D/repo/ || exit 1
RSYNC_RSH=$M rsync -a $ROOT/test/mobile $ROOT/test/fixtures hs-mac-mini:$D/repo/test/ || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$D/repo && sh scripts/mobile/ios-unit-tests.sh" > "$OUT/unit.txt" 2>&1; UNIT=$?
tail -25 "$OUT/unit.txt"; echo "unit tests: exit $UNIT"
# the hub
HOST=$(tailscale ip -4 | head -1) $RIG up "$HUBBIN" $R || exit 1
trap '$RIG down $R >/dev/null' EXIT
PORT=$(sed -n 's/^PORT=//p' $R/env); HOST=$(sed -n 's/^HOST=//p' $R/env)
sed -n 's/^TOKEN=//p' $R/env > $OUT/token; RSYNC_RSH=$M rsync -a $OUT/token hs-mac-mini:$D/token; rm -f $OUT/token
RSYNC_RSH=$M rsync -a --delete $ROOT/test/e2e/ios/ hs-mac-mini:$D/e2e/
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=\$(xcrun simctl create bp-swarm-ios 'iPhone 17' com.apple.CoreSimulator.SimRuntime.iOS-26-5) || exit 1; echo sim \$U
xcrun simctl boot \$U; xcrun simctl bootstatus \$U -b >/dev/null 2>&1
rm -rf ~/$D/out; cd ~/$D/e2e && SIM_UDID=\$U OUT=\$HOME/$D/out HUB_HOST=$HOST HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/$D/token \
  APP=\$HOME/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/$D/e2e-dd sh ./run.sh swarm; cat ~/$D/out/swarm.assert.txt
xcrun simctl shutdown \$U; xcrun simctl delete \$U; rm -f ~/$D/token"
RSYNC_RSH=$M rsync -a --exclude '*.xcresult' --exclude '*.mp4' hs-mac-mini:$D/out/ $OUT/
cp $R/hub.log $OUT/hub.log 2>/dev/null
python3 -c "
import json;j=json.load(open('$OUT/swarm.json'));print('swarm', j['status'], j.get('notes', [])[:-1]);[print(' ', s['do'], s['ok']) for s in j['metrics']['steps']]
raise SystemExit(0 if j['status']=='pass' and $UNIT == 0 else 1)"
