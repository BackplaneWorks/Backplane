#!/bin/sh
# Build the iOS app with Xcode, on a Mac (docs/ios-build.md).
#   mobile/ios/build.sh sim       Debug, installed and launched on a booted iPhone Simulator
#   mobile/ios/build.sh device    Debug, signed for development, installed on a connected iPhone
#                                 (DEVICE=<id from `xcrun devicectl list devices`>, else the first)
#   mobile/ios/build.sh archive   a signed App Store archive, uploaded to TestFlight (BUILD_NUMBER=<n>
#                                 sets the build number, default the date and minute)
# device and archive read signing.env beside this script (never committed;
# mobile/README.md): TEAM_ID and BUNDLE_ID, and for archive the App Store
# Connect key (ASC_KEY_ID, ASC_ISSUER_ID, ASC_KEY_PATH).
#
# The shipped app is the data/1 native app (DataApp, -DDATA_APP, GPUI_NATIVE):
# no Bend bridge.js, no engine. Every mode builds it.
set -eu
cd "$(dirname "$0")"
MODE=${1:-sim}
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
CONDS="GPUI_NATIVE DATA_APP"

signing() {
  [ -f signing.env ] || { echo "no signing.env beside $0 (see mobile/README.md)"; exit 1; }
  . ./signing.env
  : "${TEAM_ID:?signing.env sets TEAM_ID}" "${BUNDLE_ID:=dev.backplane.mobile}"
}

case $MODE in
sim)
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug \
    -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build/dd SWIFT_ACTIVE_COMPILATION_CONDITIONS="DEBUG $CONDS" build
  app=build/dd/Build/Products/Debug-iphonesimulator/Backplane.app
  dev=$(xcrun simctl list devices booted | grep -o '[0-9A-F-]\{36\}' | head -1)
  if [ -z "$dev" ]; then
    dev=$(xcrun simctl list devices available | grep 'iPhone' | grep -o '[0-9A-F-]\{36\}' | head -1)
    xcrun simctl boot "$dev"
  fi
  xcrun simctl install "$dev" "$app"
  xcrun simctl launch "$dev" dev.backplane.mobile
  echo "simulator $dev: $app"
  ;;
device)
  signing
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug \
    -destination 'generic/platform=iOS' -derivedDataPath build/dd -allowProvisioningUpdates \
    DEVELOPMENT_TEAM="$TEAM_ID" BASE_BUNDLE_ID="$BUNDLE_ID" build
  app=build/dd/Build/Products/Debug-iphoneos/Backplane.app
  dev=${DEVICE:-$(xcrun devicectl list devices 2>/dev/null | grep -o '[0-9A-F]\{8\}-[0-9A-F-]\{27\}' | head -1)}
  [ -n "$dev" ] || { echo "no iPhone found: connect one (and trust this Mac), or set DEVICE"; exit 1; }
  xcrun devicectl device install app --device "$dev" "$app"
  xcrun devicectl device process launch --device "$dev" "$BUNDLE_ID" || true
  echo "iPhone $dev: $app"
  ;;
archive)
  signing
  auth="-allowProvisioningUpdates -authenticationKeyPath $ASC_KEY_PATH -authenticationKeyID $ASC_KEY_ID -authenticationKeyIssuerID $ASC_ISSUER_ID"
  n=${BUILD_NUMBER:-$(date +%Y%m%d%H%M)}
  # ~/.bp-sign (a keychain password and App Store profiles, never
  # committed): sign by hand with the Distribution identity in the
  # backplane-signing keychain, which works over SSH; else let Xcode sign
  kc=$HOME/Library/Keychains/backplane-signing.keychain-db
  if [ -f "$HOME/.bp-sign/kcpass" ] && [ -f "$kc" ]; then
    security unlock-keychain -p "$(cat "$HOME/.bp-sign/kcpass")" "$kc"
    set -- CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY="Apple Distribution" OTHER_CODE_SIGN_FLAGS="--keychain $kc" \
      'PROVISIONING_PROFILE_SPECIFIER=$(BP_PROFILE_$(TARGET_NAME))' \
      BP_PROFILE_Backplane="Backplane AppStore" BP_PROFILE_BackplaneIsland="Backplane Island AppStore"
    style="<key>signingStyle</key><string>manual</string>
  <key>signingCertificate</key><string>Apple Distribution</string>
  <key>provisioningProfiles</key><dict>
    <key>$BUNDLE_ID</key><string>Backplane AppStore</string>
    <key>$BUNDLE_ID.island</key><string>Backplane Island AppStore</string>
  </dict>"
  else
    set --
    style="<key>signingStyle</key><string>automatic</string>"
  fi
  rm -rf build/Backplane.xcarchive
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Release \
    -destination 'generic/platform=iOS' -archivePath build/Backplane.xcarchive -derivedDataPath build/dd \
    DEVELOPMENT_TEAM="$TEAM_ID" BASE_BUNDLE_ID="$BUNDLE_ID" CURRENT_PROJECT_VERSION="$n" \
    SWIFT_ACTIVE_COMPILATION_CONDITIONS="$CONDS" \
    "$@" $auth archive
  cat > build/export.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>upload</string>
  <key>teamID</key><string>$TEAM_ID</string>
  $style
  <key>manageAppVersionAndBuildNumber</key><false/>
</dict></plist>
PLIST
  rm -rf build/export
  xcodebuild -exportArchive -archivePath build/Backplane.xcarchive -exportOptionsPlist build/export.plist \
    -exportPath build/export $auth
  echo "uploaded build $n to App Store Connect (TestFlight)"
  ;;
*)
  echo "usage: $0 [sim|device|archive]"; exit 2
  ;;
esac
