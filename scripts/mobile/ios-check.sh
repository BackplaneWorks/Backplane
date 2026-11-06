#!/bin/sh
# Compile-check the iOS data app on the Mac (own folder there, simulator Debug, DATA_APP): rsync mobile/ios, xcodebuild, print the errors. Nothing is run or installed.
# usage: scripts/mobile/ios-check.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout and ~/bp-gpui-ios/bridge.js on the Mac)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; M=$MAIN/build/gpui/mac-ssh; D=bp-gpui-ios-mobile
RSYNC_RSH=$M rsync -a --delete --exclude build --exclude Backplane/bridge.js $ROOT/mobile/ios/ hs-mac-mini:$D/ios/ || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/shell/backplane-app.js hs-mac-mini:$D/ios/Backplane/backplane-app.js || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/'$D'/ios || exit 1
cp ~/bp-gpui-ios/bridge.js Backplane/bridge.js
S=$(date +%s)
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination "generic/platform=iOS Simulator" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS="GPUI_NATIVE DEBUG" build 2>&1 | grep -E "error:|warning: .*Data|BUILD|\*\* " | head -60
echo "build seconds: $(( $(date +%s) - S ))"
'
