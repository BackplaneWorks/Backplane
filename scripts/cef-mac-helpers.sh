#!/bin/sh
# cef-mac-helpers.sh DIR HELPER_BIN: the embedded Chromium's helper apps in DIR, beside the client (viewers/cef_mac.rs): `Backplane Helper.app` and its
# `(GPU)`, `(Renderer)`, `(Plugin)`, `(Alerts)` twins (Chromium asks for those when the client runs from a bundle; DIR is then its Contents/Frameworks),
# each with its own copy of HELPER_BIN (target/<profile>/backplane-cef-helper, ~1 MB: an update refuses links) and LSUIElement (no Dock icon).
# The copies keep the linker's ad hoc signature; nothing else is signed (no Developer ID yet, as for the client).
set -eu
D=$1; H=$2; [ -f "$H" ] || { echo "cef-mac-helpers: no $H" >&2; exit 1; }
for s in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
  n="Backplane Helper$s"; id=$(printf '%s' "$s" | tr -d ' ()' | tr 'A-Z' 'a-z'); a="$D/$n.app/Contents"
  rm -rf "$D/$n.app"; mkdir -p "$a/MacOS"; cp "$H" "$a/MacOS/$n"; chmod 755 "$a/MacOS/$n"
  cat > "$a/Info.plist" <<P
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>$n</string>
<key>CFBundleIdentifier</key><string>works.backplane.gpui.helper${id:+.$id}</string>
<key>CFBundleName</key><string>$n</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>LSUIElement</key><true/>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict></plist>
P
done
