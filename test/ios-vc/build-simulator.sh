#!/bin/sh
# Run on the Mac from the dedicated source copy, under the ios-vc-mac lock.
set -eu
: "${SOURCE_COMMIT:?Set SOURCE_COMMIT to the exact source commit}"
: "${BRIDGE_SHA256:?Set BRIDGE_SHA256 to the expected bridge SHA-256}"
: "${SIM_DEVICE_TYPE:?Set SIM_DEVICE_TYPE to an available simctl device type identifier}"
: "${SIM_RUNTIME:?Set SIM_RUNTIME to an available simctl runtime identifier}"
case "$SOURCE_COMMIT" in *[!0-9a-f]*|'') echo 'Invalid source commit' >&2; exit 1;; esac
[ "${#SOURCE_COMMIT}" -eq 40 ] || { echo 'Source commit must be a full SHA-1' >&2; exit 1; }
case "$BRIDGE_SHA256" in *[!0-9a-f]*|'') echo 'Invalid bridge hash' >&2; exit 1;; esac
[ "${#BRIDGE_SHA256}" -eq 64 ] || { echo 'Bridge hash must be SHA-256' >&2; exit 1; }
[ -f Backplane.xcodeproj/project.pbxproj ] && [ -f Backplane/bridge.js ] || { echo 'Run from the dedicated mobile/ios source copy with bridge.js' >&2; exit 1; }
actual=$(shasum -a 256 Backplane/bridge.js | awk '{print $1}')
[ "$actual" = "$BRIDGE_SHA256" ] || { echo 'Bridge hash mismatch' >&2; exit 1; }
# Never select, erase, or delete an existing simulator. Every invocation creates one.
mkdir -p qualification-evidence
receipt="qualification-evidence/$(date -u +%Y%m%dT%H%M%SZ)-$$"
mkdir "$receipt"
printf '%s\n' "$SOURCE_COMMIT" > "$receipt/source-commit.txt"
printf '%s\n' "$actual" > "$receipt/bridge-sha256.txt"
xcodebuild -version > "$receipt/xcode-version.txt"
xcrun simctl list runtimes > "$receipt/runtimes.txt"
xcrun simctl list devicetypes > "$receipt/device-types.txt"
xcodebuild -project Backplane.xcodeproj -scheme Backplane -configuration Debug \
  -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath "$receipt/derived-data" build > "$receipt/build.log" 2>&1
app="$receipt/derived-data/Build/Products/Debug-iphonesimulator/Backplane.app"
[ -x "$app/Backplane" ] || { echo 'Built app executable missing' >&2; exit 1; }
shasum -a 256 "$app/Backplane" > "$receipt/app-executable-sha256.txt"
dev=$(xcrun simctl create "Backplane qualification 655983 $(date -u +%Y%m%dT%H%M%SZ)-$$" "$SIM_DEVICE_TYPE" "$SIM_RUNTIME")
printf '%s\n' "$dev" > "$receipt/simulator-udid.txt"
xcrun simctl boot "$dev"
xcrun simctl bootstatus "$dev" -b
xcrun simctl install "$dev" "$app"
printf 'Installed on new simulator %s; evidence: %s\n' "$dev" "$receipt"
printf '%s\n' 'Launch and pair separately; do not pass tokens in shell arguments or logs.'
