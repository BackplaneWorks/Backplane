#!/bin/sh
# The native iOS app to TestFlight, from the Mac (~/bp-gpui-ios/ios), signed with the existing App Store credentials (signing.env,
# ~/.bp-sign) and the bundle that builds already go to. Run under the mac-heavy lock.
#   scripts/ios-archive.sh check     a Release device build with GPUI_NATIVE, no signing, no upload (does it compile?)
#   scripts/ios-archive.sh upload    archive, sign and upload to App Store Connect; prints the build number
# The build number is the date and minute (monotonic: the last upload was 202611031753). BP_MEASURE is never set here: that flag takes a
# pairing link from the environment, and an archive must not.
cd "$(dirname "$0")/.." || exit 1
M=build/gpui/mac-ssh; MODE=${1:-check}
RSYNC_RSH=$PWD/$M rsync -a --delete --exclude build --exclude signing.env mobile/ios/ hs-mac-mini:bp-gpui-ios/ios/ || exit 1
$M hs-mac-mini "
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/bp-gpui-ios/ios || exit 1
[ -f signing.env ] || cp ~/backplane-ios/signing.env signing.env
case $MODE in
check)
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Release -sdk iphoneos -destination 'generic/platform=iOS' \
    -derivedDataPath build/ddc CODE_SIGNING_ALLOWED=NO SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DATA_APP\" build 2>&1 | tail -40; echo \"check done\";;
upload)
  n=\$(date +%Y%m%d%H%M); BUILD_NUMBER=\$n sh build.sh archive 2>&1 | tail -30; echo \"build number \$n\";;
esac"
