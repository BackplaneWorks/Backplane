#!/bin/sh
# Check the iOS app's thread grouping (mobile/ios/Backplane/ThreadGroups.swift) on the Mac:
# swiftc compiles ThreadGroups.swift with mobile/ios/Tests/ThreadGroupsTests.swift (no app, no simulator) and runs it.
# usage: scripts/mobile/ios-thread-groups-test.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-thread-groups-test}
$M hs-mac-mini "mkdir -p $D" || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/ThreadGroups.swift hs-mac-mini:$D/ || exit 1
# top-level code must be main.swift when swiftc gets several files
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Tests/ThreadGroupsTests.swift hs-mac-mini:$D/main.swift || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd $D && xcrun swiftc -O -o thread-groups-test ThreadGroups.swift main.swift && ./thread-groups-test ; r=\$?; cd && rm -rf $D; exit \$r"
