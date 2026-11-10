#!/bin/sh
# Settings › Ring on the iOS app in a simulator of its own on the Mac mini (test/e2e/ios journey `ring`, Journeys.test_ring), against test/ring/rig.py's hub on this
# machine's tailnet address. Once the phone's Set up has made the hook, this sends the Pebble app's test event and then a recording, as the app would.
# usage: test/ring/ios_ring.sh HUB_BIN VOICE_BIN LOCAL_DIR SAMPLE.m4a     Start with work_run {lock: "mac-heavy"}. Results and shots: build/mobile/ios-ring.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}
HUBBIN=$1; VOICEBIN=$2; LOCALDIR=$3; SAMPLE=$4
D=${D:-bp-ring-ios}; OUT=$ROOT/build/mobile/ios-ring; rm -rf "$OUT"; mkdir -p "$OUT"
. "$ROOT/scripts/lib-bptmp.sh"; R=$BP_TMP/ring-ios; rm -rf $R; RIG="python3 -I $ROOT/test/ring/rig.py"
M=$M D=$D "$ROOT/scripts/mobile/ios-check.sh" | tail -8
$M hs-mac-mini "test -d ~/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app" || { echo "the app did not build"; exit 1; }
HOST=$(tailscale ip -4 | head -1) $RIG up "$HUBBIN" "$VOICEBIN" "$LOCALDIR" $R || exit 1
trap '$RIG down $R >/dev/null; kill $W 2>/dev/null' EXIT
PORT=$(sed -n 's/^PORT=//p' $R/env); HOST=$(sed -n 's/^HOST=//p' $R/env)
# the Pebble app's part: once Set up made the hook, its test event, then a recording
( for i in $(seq 1 600); do ls $R/home/secrets/hooks/*.token >/dev/null 2>&1 && break; sleep 1; done; sleep 8
  echo "test: $($RIG test $R)"; sleep 20; echo "record: $($RIG record $R "$SAMPLE" rec-ios-1)" ) > $OUT/pebble.log 2>&1 & W=$!
sed -n 's/^TOKEN=//p' $R/env > $OUT/token; RSYNC_RSH=$M rsync -a $OUT/token hs-mac-mini:$D/token; rm -f $OUT/token
RSYNC_RSH=$M rsync -a --delete $ROOT/test/e2e/ios/ hs-mac-mini:$D/e2e/
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=\$(xcrun simctl create bp-ring-ios 'iPhone 17' com.apple.CoreSimulator.SimRuntime.iOS-26-5) || exit 1; echo sim \$U
xcrun simctl boot \$U; xcrun simctl bootstatus \$U -b >/dev/null 2>&1
rm -rf ~/$D/out; cd ~/$D/e2e && SIM_UDID=\$U OUT=\$HOME/$D/out HUB_HOST=$HOST HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/$D/token \
  APP=\$HOME/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/$D/e2e-dd sh ./run.sh ring; cat ~/$D/out/ring.assert.txt
xcrun simctl shutdown \$U; xcrun simctl delete \$U"
RSYNC_RSH=$M rsync -a --exclude '*.xcresult' --exclude '*.mp4' hs-mac-mini:$D/out/ $OUT/
cat $OUT/pebble.log
python3 -c "
import json;j=json.load(open('$OUT/ring.json'));print('ring', j['status'], j.get('notes', []));[print(' ', s['do'], s['ok']) for s in j['metrics']['steps']]
raise SystemExit(0 if j['status']=='pass' else 1)"
