#!/bin/sh
# Build the iOS app on a Mac over SSH (bend runs here; Xcode runs there).
#   scripts/build-ios.sh [sim|archive]     (default sim)
#   BACKPLANE_MAC=hs-mac-mini              the ssh host
#   BACKPLANE_MAC_DIR=backplane-ios        the copy on the Mac (one per
#                                          checkout when several build at once)
#   sim      a Simulator build, installed and launched on a booted iPhone
#   archive  a signed App Store archive (needs mobile/ios/signing.env there)
set -eu
cd "$(dirname "$0")/.."
MAC=${BACKPLANE_MAC:-hs-mac-mini}
MODE=${1:-sim}
DIR=${BACKPLANE_MAC_DIR:-backplane-ios}
[ -n "${BACKPLANE_SKIP_JS:-}" ] || scripts/build-mobile.sh --js
cp mobile/build/assets/bridge.js mobile/ios/Backplane/bridge.js
cp assets/fonts/NotoSansCuneiform-Regular.ttf mobile/ios/Backplane/
cp assets/fonts/NOTO-CUNEIFORM-OFL.txt mobile/ios/Backplane/
rsync -a --delete --exclude build/ --exclude authorize-mac.sh --exclude signing.env --exclude '*.p8' \
  mobile/ios/ "$MAC:$DIR/"
ssh "$MAC" "cd $DIR && MODE=$MODE sh -s" <<'REMOTE'
set -eu
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
case $MODE in
sim)
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug \
    -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build/dd build
  app=build/dd/Build/Products/Debug-iphonesimulator/Backplane.app
  dev=$(xcrun simctl list devices booted | grep -o '[0-9A-F-]\{36\}' | head -1)
  if [ -z "$dev" ]; then
    dev=$(xcrun simctl list devices available | grep 'iPhone 17 Pro (' | grep -o '[0-9A-F-]\{36\}' | head -1)
    xcrun simctl boot "$dev"
  fi
  xcrun simctl install "$dev" "$app"
  xcrun simctl launch "$dev" dev.backplane.mobile
  echo "simulator $dev: $app"
  ;;
archive)
  [ -f signing.env ] || { echo "no signing.env on the Mac (see mobile/README.md)"; exit 1; }
  . ./signing.env
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
esac
REMOTE
