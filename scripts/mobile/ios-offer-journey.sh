#!/bin/sh
# The home hub's offer and failover on the iOS Simulator (scripts/mobile/ios-offer-mac.sh on the Mac, its own folder and Simulator); the shots come back to OUT.
# usage: scripts/mobile/ios-offer-journey.sh [OUT]      (hold the mac-heavy lock; needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}
D=${D:-bp-hub-offer-ios}; OUT=${1:-${BP_TMP:-$HOME/.cache/bp-tmp}/ios-offer}; SIM=${SIM:-bp-hub-offer}
RSYNC_RSH=$M rsync -a --delete --exclude build $ROOT/mobile/ios/ hs-mac-mini:$D/ios/ || exit 1
RSYNC_RSH=$M rsync -a $ROOT/test/mobile/offer_stub.py $ROOT/scripts/mobile/ios-offer-mac.sh hs-mac-mini:$D/ || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/'$D'/ios && xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination "generic/platform=iOS Simulator" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS="GPUI_NATIVE DEBUG" build 2>&1 | grep -E "error:" | head -30
U=$(xcrun simctl list devices | grep "'$SIM' (" | head -1 | sed -E "s/.*\(([0-9A-F-]{36})\).*/\1/")
[ -n "$U" ] || U=$(xcrun simctl create '$SIM' "iPhone 17")
sh ~/'$D'/ios-offer-mac.sh ~/'$D' "$U"; r=$?; xcrun simctl shutdown "$U" 2>/dev/null; exit $r
'; r=$?
mkdir -p "$OUT"; RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ "$OUT/"
echo "shots in $OUT"; exit $r
