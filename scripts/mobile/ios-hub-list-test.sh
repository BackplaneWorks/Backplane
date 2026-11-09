#!/bin/sh
# Check the iOS app's paired-hub list and invitation addresses (mobile/ios/Backplane/HubList.swift) on the Mac:
# swiftc compiles HubList.swift with mobile/ios/Tests/HubListTests.swift (no app, no simulator) and runs it.
# usage: scripts/mobile/ios-hub-list-test.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(git -C "$ROOT" worktree list --porcelain | awk 'NR==1{print substr($0,10)}')}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-hub-list-test}
$M hs-mac-mini "mkdir -p $D" || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/HubList.swift $ROOT/test/mobile/hubs_cases.json hs-mac-mini:$D/ || exit 1
# top-level code must be main.swift when swiftc gets several files
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Tests/HubListTests.swift hs-mac-mini:$D/main.swift || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd $D && xcrun swiftc -O -o hub-list-test HubList.swift main.swift && ./hub-list-test hubs_cases.json ; r=\$?; cd && rm -rf $D; exit \$r"
