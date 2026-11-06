#!/bin/sh
# Build the GPUI iOS app on the Mac (run under the mac-heavy lock). Usage: [ADHOC=1] ios-build.sh [clean|release]
# DATA=1: the data/1 app (DataApp is @main; no engine, no bridge). ADHOC=1: the Debug simulator build is ad-hoc signed so its entitlements (the keychain, which holds a pairing) are embedded; unsigned,
# every keychain call fails with -34018 and a pairing made from an invitation is lost
# release: an optimised (Release) simulator build with GPUI_NATIVE and BP_MEASURE (the measurement hooks: the pairing link from the
# environment, BPPerf and Boot marks), in build/ddr; the Debug build stays in build/dd.
cd "$(dirname "$0")/.." || exit 1
M=build/gpui/mac-ssh
RSYNC_RSH=$PWD/$M rsync -a --delete --exclude build --exclude Backplane/bridge.js mobile/ios/ hs-mac-mini:bp-gpui-ios/ios/ || exit 1
RSYNC_RSH=$PWD/$M rsync -a mobile/shell/backplane-app.js hs-mac-mini:bp-gpui-ios/ios/Backplane/backplane-app.js || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/bp-gpui-ios/ios || exit 1
SIGNARGS="CODE_SIGNING_ALLOWED=NO"; [ "'"$ADHOC"'" = 1 ] && SIGNARGS="CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=-"
CONDS=""; [ "'"$DATA"'" = 1 ] && CONDS="OTHER_SWIFT_FLAGS=-DDATA_APP"
[ "'"$1"'" = clean ] && rm -rf build/dd
S=$(date +%s)
if [ "'"$1"'" = release ]; then
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Release -sdk iphonesimulator \
  -destination "generic/platform=iOS Simulator" -derivedDataPath build/ddr CODE_SIGNING_ALLOWED=NO $CONDS \
  SWIFT_ACTIVE_COMPILATION_CONDITIONS="GPUI_NATIVE BP_MEASURE" build 2>&1 | tail -60
du -sh build/ddr/Build/Products/Release-iphonesimulator/Backplane.app
else
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator \
  -destination "generic/platform=iOS Simulator" -derivedDataPath build/dd $SIGNARGS $CONDS build 2>&1 | tail -60
du -sh build/dd/Build/Products/Debug-iphonesimulator/Backplane.app
fi
echo "build seconds: $(( $(date +%s) - S ))"
'
