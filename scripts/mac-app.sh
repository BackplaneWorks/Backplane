#!/bin/sh
# mac-app.sh DIR VERSION: Backplane.app in DIR, the app deploy/install.sh.in puts in ~/Applications (docs/install.md). Its program is
# deploy/mac/launcher.c, which execs the client of the build the guard runs (<prefix>/CURRENT), so the app outlives updates unchanged.
# Info.plist: works.backplane.gpui (the helpers are works.backplane.gpui.helper*, scripts/cef-mac-helpers.sh), VERSION (the tag without its v),
# macOS 13 or later, arm64, the usage descriptions macOS shows in its privacy prompts; the icon is assets/icon/backplane-desktop-512.png as an .icns.
# Ad hoc signed (codesign -s -): no Developer ID yet. macOS only (clang, sips, iconutil, codesign).
set -eu
D=$1; V=${2#v}; R=$(cd "$(dirname "$0")/.." && pwd)
A="$D/Backplane.app/Contents"; rm -rf "$D/Backplane.app"; mkdir -p "$A/MacOS" "$A/Resources"
clang -O2 -arch arm64 -mmacosx-version-min=13.0 -framework CoreFoundation -o "$A/MacOS/Backplane" "$R/deploy/mac/launcher.c"
# the icon: every size iconutil takes, from the 512 px desktop icon (1024 for 512@2x: upscaled, as the art has no larger raster)
src="$R/assets/icon/backplane-desktop-512.png"; t=$(mktemp -d); s="$t/Backplane.iconset"; mkdir "$s"
for n in 16 32 128 256 512; do
  sips -z $n $n "$src" --out "$s/icon_${n}x${n}.png" >/dev/null
  sips -z $((n * 2)) $((n * 2)) "$src" --out "$s/icon_${n}x${n}@2x.png" >/dev/null
done
iconutil -c icns "$s" -o "$A/Resources/Backplane.icns"; rm -rf "$t"
cat > "$A/Info.plist" <<P
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>Backplane</string>
<key>CFBundleIdentifier</key><string>works.backplane.gpui</string>
<key>CFBundleName</key><string>Backplane</string>
<key>CFBundleDisplayName</key><string>Backplane</string>
<key>CFBundleIconFile</key><string>Backplane</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleShortVersionString</key><string>${V%%-*}</string>
<key>CFBundleVersion</key><string>$V</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>LSArchitecturePriority</key><array><string>arm64</string></array>
<key>LSRequiresNativeExecution</key><true/>
<key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
<key>CFBundleURLTypes</key><array><dict><key>CFBundleURLName</key><string>Pairing</string><key>CFBundleURLSchemes</key><array><string>backplane</string></array></dict></array>
<key>NSDocumentsFolderUsageDescription</key><string>Backplane opens your projects in your Documents folder.</string>
<key>NSDesktopFolderUsageDescription</key><string>Backplane opens your projects on your Desktop.</string>
<key>NSDownloadsFolderUsageDescription</key><string>Backplane opens your projects and the files you download in your Downloads folder.</string>
<key>NSRemovableVolumesUsageDescription</key><string>Backplane opens your projects on removable drives.</string>
<key>NSNetworkVolumesUsageDescription</key><string>Backplane opens your projects on network drives.</string>
<key>NSMicrophoneUsageDescription</key><string>Backplane listens to your voice when you dictate a message.</string>
</dict></plist>
P
plutil -lint "$A/Info.plist" >/dev/null
codesign --force -s - "$D/Backplane.app"
