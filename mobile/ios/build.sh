#!/bin/sh
# Build the iOS app with Xcode, on a Mac (docs/ios-build.md).
#   mobile/ios/build.sh sim       Debug, installed and launched on a booted iPhone Simulator
#   mobile/ios/build.sh device    Debug, signed for development, installed on a connected iPhone
#                                 (DEVICE=<id from `xcrun devicectl list devices`>, else the first)
#   mobile/ios/build.sh archive   a signed App Store archive, uploaded to TestFlight (BUILD_NUMBER=<n>
#                                 sets the build number, default the date and minute)
# MARKETING_VERSION or BACKPLANE_VERSION overrides the app version; otherwise it is synced from the
# hub version (native/host/Cargo.toml / release git tags) when in a repo checkout, or project.pbxproj.
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

normalize_version() {
  raw=$1
  v=${raw#v}
  v=${v%%-*}
  case "$v" in
    ''|*[!0-9.]*|.*|*.|*..*) return 1 ;;
  esac
  old_ifs=$IFS; IFS=.
  set -- $v
  IFS=$old_ifs
  [ $# -eq 2 ] || [ $# -eq 3 ] || return 1
  printf '%s\n' "$v"
}

semver_gt() {
  lhs=$1 rhs=$2
  old_ifs=$IFS; IFS=.
  set -- $lhs
  a1=${1:-0} a2=${2:-0} a3=${3:-0}
  set -- $rhs
  b1=${1:-0} b2=${2:-0} b3=${3:-0}
  IFS=$old_ifs
  for n in a1 a2 a3 b1 b2 b3; do
    eval "val=\$$n"
    while case "$val" in 0[0-9]*) val=${val#0} ;; *) false ;; esac; do :; done
    eval "$n=\$val"
  done
  [ "$a1" -gt "$b1" ] && return 0
  [ "$a1" -lt "$b1" ] && return 1
  [ "$a2" -gt "$b2" ] && return 0
  [ "$a2" -lt "$b2" ] && return 1
  [ "$a3" -gt "$b3" ] && return 0
  return 1
}

resolve_marketing_version() {
  if [ -n "${MARKETING_VERSION:-}" ]; then
    normalize_version "$MARKETING_VERSION" || { echo "invalid MARKETING_VERSION: $MARKETING_VERSION" >&2; return 1; }
    return 0
  fi
  if [ -n "${BACKPLANE_VERSION:-}" ]; then
    normalize_version "$BACKPLANE_VERSION" || { echo "invalid BACKPLANE_VERSION: $BACKPLANE_VERSION" >&2; return 1; }
    return 0
  fi

  if [ -f "../../native/host/Cargo.toml" ]; then
    head_tags=$(git -C ../.. tag --points-at HEAD 'v[0-9]*' 2>/dev/null || true)
    best_stable=""
    best_pre=""
    for tag in $head_tags; do
      case "$tag" in
        *-*)
          norm=$(normalize_version "$tag" 2>/dev/null) || continue
          if [ -z "$best_pre" ] || semver_gt "$norm" "$best_pre"; then best_pre=$norm; fi
          ;;
        *)
          norm=$(normalize_version "$tag" 2>/dev/null) || continue
          if [ -z "$best_stable" ] || semver_gt "$norm" "$best_stable"; then best_stable=$norm; fi
          ;;
      esac
    done
    if [ -n "$best_stable" ]; then printf '%s\n' "$best_stable"; return 0; fi
    if [ -n "$best_pre" ]; then printf '%s\n' "$best_pre"; return 0; fi

    best=""
    raw_cargo=$(awk '
      /^\[/ { in_pkg = ($0 ~ /^\[package\]([[:space:]]*(#.*)?)?$/) }
      in_pkg && /^[[:space:]]*version[[:space:]]*=/ {
        sub(/^[^"]*"/, ""); sub(/".*$/, ""); print; exit
      }
    ' "../../native/host/Cargo.toml")
    if [ -n "$raw_cargo" ]; then
      best=$(normalize_version "$raw_cargo") || { echo "invalid version '$raw_cargo' in native/host/Cargo.toml" >&2; return 1; }
    fi
    rel_tags=$(git -C ../.. tag -l 'v[0-9]*.[0-9]*.[0-9]*' 2>/dev/null || true)
    for tag in $rel_tags; do
      case "$tag" in *-*) continue ;; esac
      norm=$(normalize_version "$tag" 2>/dev/null) || continue
      if [ -z "$best" ] || semver_gt "$norm" "$best"; then best=$norm; fi
    done
    [ -n "$best" ] && { printf '%s\n' "$best"; return 0; }
  fi

  pbx_ver=$(sed -n 's/.*MARKETING_VERSION = \([^;]*\);.*/\1/p' Backplane.xcodeproj/project.pbxproj | head -n 1)
  normalize_version "$pbx_ver" || { echo "could not determine valid MARKETING_VERSION (pbxproj='$pbx_ver')" >&2; return 1; }
}

signing() {
  [ -f signing.env ] || { echo "no signing.env beside $0 (see mobile/README.md)"; exit 1; }
  . ./signing.env
  : "${TEAM_ID:?signing.env sets TEAM_ID}" "${BUNDLE_ID:=dev.backplane.mobile}"
}

case $MODE in
  sim|device|archive) ;;
  *) echo "usage: $0 [sim|device|archive]" >&2; exit 2 ;;
esac

ver=$(resolve_marketing_version)

if [ "${SKIP_SOLIDCRAFT_WEB_BUNDLE:-0}" != 1 ]; then
  sh ./stage-solidcraft-web.sh
fi

case $MODE in
sim)
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug \
    -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build/dd \
    MARKETING_VERSION="$ver" SWIFT_ACTIVE_COMPILATION_CONDITIONS="DEBUG $CONDS" build
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
    DEVELOPMENT_TEAM="$TEAM_ID" BASE_BUNDLE_ID="$BUNDLE_ID" MARKETING_VERSION="$ver" build
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
    DEVELOPMENT_TEAM="$TEAM_ID" BASE_BUNDLE_ID="$BUNDLE_ID" MARKETING_VERSION="$ver" CURRENT_PROJECT_VERSION="$n" \
    SWIFT_ACTIVE_COMPILATION_CONDITIONS="$CONDS" \
    "$@" $auth archive
  for plist in \
    build/Backplane.xcarchive/Products/Applications/Backplane.app/Info.plist \
    build/Backplane.xcarchive/Products/Applications/Backplane.app/PlugIns/BackplaneIsland.appex/Info.plist
  do
    [ -f "$plist" ] || { echo "missing archive Info.plist: $plist" >&2; exit 1; }
    got_short=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist") || { echo "missing CFBundleShortVersionString in $plist" >&2; exit 1; }
    got_build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$plist") || { echo "missing CFBundleVersion in $plist" >&2; exit 1; }
    [ "$got_short" = "$ver" ] || { echo "version mismatch in $plist: got $got_short, want $ver" >&2; exit 1; }
    [ "$got_build" = "$n" ] || { echo "build mismatch in $plist: got $got_build, want $n" >&2; exit 1; }
  done
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
  echo "uploaded Backplane $ver ($n) to App Store Connect (TestFlight)"
  ;;
*)
  echo "usage: $0 [sim|device|archive]"; exit 2
  ;;
esac
