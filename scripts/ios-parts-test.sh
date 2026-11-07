#!/bin/sh
# Check the iOS app's board-model parts (mobile/ios/Backplane/PlotParts.swift) on the Mac: swiftc compiles it with Plot.swift and
# mobile/ios/Tests/PlotPartsTests.swift (no app, no simulator) and runs it.
# usage: scripts/ios-parts-test.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd); MAIN=${MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-parts-test}
$M hs-mac-mini "mkdir -p $D" || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/Plot.swift $ROOT/mobile/ios/Backplane/PlotParts.swift hs-mac-mini:$D/ || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Tests/PlotPartsTests.swift hs-mac-mini:$D/main.swift || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd $D && xcrun swiftc -O -o parts-test Plot.swift PlotParts.swift main.swift && ./parts-test ; r=\$?; cd && rm -rf $D; exit \$r"
