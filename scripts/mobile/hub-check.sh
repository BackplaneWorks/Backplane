#!/bin/sh
# iOS's HubRescue.swift against test/mobile/hub_cases.json (the table Android's HubRescueTest reads) and test/mobile/hub_stub.py over real HTTP. The Xcode project has no test target: swiftc on the Mac, as fold-check.sh does.
# usage: scripts/mobile/hub-check.sh       (needs the ssh wrapper build/gpui/mac-ssh of the main checkout, or M=<ssh command>; PORT=<free port on the Mac>)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-hub-check}; PORT=${PORT:-3871}
TOKEN=0123456789abcdef0123456789abcdef
$M hs-mac-mini "mkdir -p $D"
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/HubRescue.swift $ROOT/test/mobile/hub_cases.json $ROOT/test/mobile/hub_stub.py hs-mac-mini:$D/
RSYNC_RSH=$M rsync -a $ROOT/test/mobile/hub_rescue_check.swift hs-mac-mini:$D/main.swift
$M hs-mac-mini "cd $D && export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer && xcrun swiftc -o hubcheck HubRescue.swift main.swift 2>&1 | grep -v warning | head -30
python3 hub_stub.py $PORT $TOKEN & S=\$!; trap 'kill \$S' EXIT; sleep 1; ./hubcheck $PORT $TOKEN"
