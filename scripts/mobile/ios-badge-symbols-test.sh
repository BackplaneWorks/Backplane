#!/bin/sh
# Check the iOS app's project icons (mobile/ios/Backplane/BadgeSymbols.swift) on the Mac: every icon of the hub's list has an SF Symbol that exists there.
# usage: scripts/mobile/ios-badge-symbols-test.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-badge-symbols-test}
$M hs-mac-mini "mkdir -p $D" || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/BadgeSymbols.swift $ROOT/native/host/src/present/badge.rs hs-mac-mini:$D/ || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Tests/BadgeSymbolsTests.swift hs-mac-mini:$D/main.swift || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd $D && xcrun swiftc -O -o badge-symbols-test BadgeSymbols.swift main.swift && ./badge-symbols-test badge.rs ; r=\$?; cd && rm -rf $D; exit \$r"
