#!/bin/sh
# Dedicated Mac iOS source copy only. No install, launch, archive or export.
set -eu
: "${SOURCE_COMMIT:?Set SOURCE_COMMIT to the full source commit}"
: "${BRIDGE_SHA256:?Set BRIDGE_SHA256 to the bridge SHA-256}"
case "$SOURCE_COMMIT" in *[!0-9a-f]*|'') echo 'Invalid source commit' >&2; exit 1;; esac
[ "${#SOURCE_COMMIT}" -eq 40 ] || { echo 'Source commit must be a full SHA-1' >&2; exit 1; }
case "$BRIDGE_SHA256" in *[!0-9a-f]*|'') echo 'Invalid bridge hash' >&2; exit 1;; esac
[ "${#BRIDGE_SHA256}" -eq 64 ] || { echo 'Bridge hash must be SHA-256' >&2; exit 1; }
mode=${1:-sim}
case "$mode" in sim|iphoneos-unsigned) ;; *) echo 'Usage: validate-local-release.sh [sim|iphoneos-unsigned]' >&2; exit 1;; esac
case "$(pwd -P)" in
    */backplane-ios-vc-655983*|*/backplane-ios-vc-655983*/mobile/ios) ;;
    *) echo 'Refusing to build outside a dedicated source copy' >&2; exit 1;;
esac
[ -f Backplane.xcodeproj/project.pbxproj ] && [ -f Backplane/bridge.js ] || { echo 'Run from the dedicated iOS source copy' >&2; exit 1; }
actual=$(shasum -a 256 Backplane/bridge.js | awk '{print $1}')
[ "$actual" = "$BRIDGE_SHA256" ] || { echo 'Bridge hash mismatch' >&2; exit 1; }
mkdir -p qualification-evidence
receipt=$(mktemp -d qualification-evidence/local-release.XXXXXXXX)
printf '%s\n' "$SOURCE_COMMIT" > "$receipt/source-commit-asserted.txt"
printf '%s\n' "$actual" > "$receipt/bridge-sha256.txt"
printf '%s\n' "$mode" > "$receipt/mode.txt"
xcodebuild -version > "$receipt/xcode-version.txt"
if [ "$mode" = sim ]; then
    sdk=iphonesimulator
    destination='generic/platform=iOS Simulator'
else
    sdk=iphoneos
    destination='generic/platform=iOS'
fi
xcodebuild -project Backplane.xcodeproj -scheme Backplane -configuration Release \
    -sdk "$sdk" -destination "$destination" -derivedDataPath "$receipt/derived-data" \
    CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build > "$receipt/build.log" 2>&1
app="$receipt/derived-data/Build/Products/Release-$sdk/Backplane.app"
[ -x "$app/Backplane" ] || { echo 'Built app executable missing' >&2; exit 1; }
shasum -a 256 "$app/Backplane" > "$receipt/app-executable-sha256.txt"
xcrun vtool -show-build "$app/Backplane" > "$receipt/app-platform.txt" 2>&1
# Unsigned products may have no embedded entitlements; retain tool output as evidence.
if ! codesign -d --entitlements - "$app" > "$receipt/app-entitlements.plist" 2> "$receipt/entitlements-status.txt"; then
    printf '%s\n' 'No readable embedded signature/entitlements in this unsigned build.' >> "$receipt/entitlements-status.txt"
fi
printf 'Local unsigned %s build complete; evidence: %s\n' "$mode" "$receipt"
printf '%s\n' 'Source commit is caller-asserted; verify the archived source provenance separately.'
