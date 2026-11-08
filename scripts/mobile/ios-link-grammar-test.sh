#!/bin/sh
# Check the phones' link grammar (mobile/ios/Backplane/DataLinkGrammar.swift) on the Mac: swiftc with mobile/ios/Tests/LinkGrammarTests.swift (no app, no simulator), runs it.
# usage: scripts/mobile/ios-link-grammar-test.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-link-grammar-test}
$M hs-mac-mini "mkdir -p $D" || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/DataLinkGrammar.swift hs-mac-mini:$D/ || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Tests/LinkGrammarTests.swift hs-mac-mini:$D/main.swift || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd $D && xcrun swiftc -O -o link-grammar-test DataLinkGrammar.swift main.swift && ./link-grammar-test ; r=\$?; cd && rm -rf $D; exit \$r"
