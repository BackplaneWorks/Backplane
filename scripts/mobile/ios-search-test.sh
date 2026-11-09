#!/bin/sh
# The iOS data app's search and find journey (test/e2e/ios data-search) on the simulator against a hub from scripts/mobile/data-hub.sh started with SEARCH=1
# and HOST=<this machine's tailnet address>; first ThreadFind's unit test (swiftc, no app). Screenshots and the result land in build/mobile/ios-search.
# usage: HUBLINK=http://<host>:<port>/#token=… scripts/mobile/ios-search-test.sh      Start with work_run {lock: "mac-heavy"}; builds the app on the Mac first.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}
U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; D=${D:-bp-gpui-ios-search}; OUT=$ROOT/build/mobile/ios-search; mkdir -p $OUT
HOSTPORT=$(echo "$HUBLINK" | sed 's|http://||; s|/.*||'); TOK=$(echo "$HUBLINK" | sed 's|.*token=||')
M=$M D=$D "$ROOT/scripts/mobile/ios-check.sh" | tail -8
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; mkdir -p ~/$D/tf && cd ~/$D/tf && cp ../ios/Backplane/ThreadFind.swift . && cp ../ios/Tests/ThreadFindTests.swift main.swift && xcrun swiftc -O -o t ThreadFind.swift main.swift && ./t"
printf %s "$TOK" > $OUT/token; RSYNC_RSH=$M rsync -a $OUT/token hs-mac-mini:$D/token
RSYNC_RSH=$M rsync -a --delete $ROOT/test/e2e/ios/ hs-mac-mini:$D/e2e/
$M hs-mac-mini "cd ~/$D/e2e && rm -rf ~/$D/out && SIM_UDID=$U OUT=\$HOME/$D/out HUB_HOST=${HOSTPORT%:*} HUB_PORT=${HOSTPORT#*:} HUB_TOKEN_FILE=\$HOME/$D/token APP=\$HOME/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/$D/e2e-dd sh ./run.sh data-search; cat ~/$D/out/data-search.assert.txt; xcrun simctl shutdown $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ $OUT/
python3 -c "
import json;j=json.load(open('$OUT/data-search.json'));print(j['status']);[print(s['do'],s['ok'],s['result_ms']) for s in j['metrics']['steps']]"
