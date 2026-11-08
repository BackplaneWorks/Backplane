#!/bin/sh
# The iOS data app's link journey (test/e2e/ios data-links) on the simulator against a hub from scripts/mobile/data-hub.sh started with LINKS=1 and HOST=<this machine's tailnet address>.
# usage: HUBLINK=http://<host>:<port>/#token=… scripts/mobile/ios-links-test.sh      Start with work_run {lock: "mac-heavy"}; builds the app on the Mac first.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); M=${M:-$ROOT/../../../build/gpui/mac-ssh}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}
D=${D:-bp-gpui-ios-links}; OUT=$ROOT/build/mobile/ios-links; mkdir -p $OUT
HOSTPORT=$(echo "$HUBLINK" | sed 's|http://||; s|/.*||'); TOK=$(echo "$HUBLINK" | sed 's|.*token=||')
M=$M D=$D "$ROOT/scripts/mobile/ios-check.sh" | tail -5
printf %s "$TOK" > $OUT/token; RSYNC_RSH=$M rsync -a $OUT/token hs-mac-mini:$D/token
RSYNC_RSH=$M rsync -a --delete $ROOT/test/e2e/ios/ hs-mac-mini:$D/e2e/
$M hs-mac-mini "cd ~/$D/e2e && rm -rf ~/$D/out && SIM_UDID=$U OUT=\$HOME/$D/out HUB_HOST=${HOSTPORT%:*} HUB_PORT=${HOSTPORT#*:} HUB_TOKEN_FILE=\$HOME/$D/token APP=\$HOME/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app sh ./run.sh data-links; cat ~/$D/out/data-links.assert.txt; xcrun simctl shutdown $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ $OUT/
python3 -c "
import json;j=json.load(open('$OUT/data-links.json'));print(j['status']);[print(s['do'],s['ok']) for s in j['metrics']['steps']]"
