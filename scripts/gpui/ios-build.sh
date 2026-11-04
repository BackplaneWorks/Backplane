#!/bin/sh
# Build the GPUI iOS app on the Mac (run under the mac-heavy lock). Usage: ios-build.sh [clean]
cd "$(dirname "$0")/../.." || exit 1
M=build/gpui/mac-ssh
RSYNC_RSH=$PWD/$M rsync -a --delete --exclude build --exclude Backplane/bridge.js mobile/ios/ hs-mac-mini:bp-gpui-ios/ios/ || exit 1
RSYNC_RSH=$PWD/$M rsync -a mobile/shell/backplane-app.js hs-mac-mini:bp-gpui-ios/ios/Backplane/backplane-app.js || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/bp-gpui-ios/ios || exit 1
cp ~/bp-gpui-ios/bridge.js Backplane/bridge.js
[ "'"$1"'" = clean ] && rm -rf build/dd
S=$(date +%s)
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator \
  -destination "generic/platform=iOS Simulator" -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO build 2>&1 | tail -40
echo "build seconds: $(( $(date +%s) - S ))"
du -sh build/dd/Build/Products/Debug-iphonesimulator/Backplane.app
'
