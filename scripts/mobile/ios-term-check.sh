#!/bin/sh
# The iOS app's unit tests, then a compile of the data app, on the Mac inside its heavy lock. Run under the Backplane lock mac-heavy.
# usage: scripts/mobile/ios-term-check.sh
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-iterm}
RSYNC_RSH=$M rsync -a --delete --exclude build $ROOT/mobile/ios/ hs-mac-mini:$D/ios/ || exit 1
RSYNC_RSH=$M rsync -a --delete $ROOT/scripts/mobile/ios-unit-tests.sh hs-mac-mini:$D/unit.sh || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/'$D'/ios || exit 1
lockf -k /tmp/bp-mac-heavy.lockf sh -c "
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build 2>&1 | grep -E \"error:|BUILD|\*\* \" | head -60"
'
