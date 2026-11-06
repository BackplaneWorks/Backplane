#!/bin/sh
# Run the Mac build in the logged-in GUI session via an .app bundle (ssh has no window server).
# usage: scripts/mac-run.sh LOGNAME [client args...]   -> logs in ~/bp-gpui-desktop/logs/LOGNAME.{out,err}
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd); L=$1; shift
exec "$ROOT/build/gpui/mac-ssh" hs-mac-mini "sh -s" -- "$L" "$@" <<'R'
set -eu
L=$1; shift; cd ~/bp-gpui-desktop
A=BackplaneGPUI.app; mkdir -p $A/Contents/MacOS
cp dist/backplane-gpui $A/Contents/MacOS/backplane-gpui
cat > $A/Contents/Info.plist <<P
<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>backplane-gpui</string><key>CFBundleURLTypes</key><array><dict><key>CFBundleURLName</key><string>Pairing</string><key>CFBundleURLSchemes</key><array><string>backplane</string></array></dict></array><key>CFBundleIdentifier</key><string>works.backplane.gpui.experiment</string><key>CFBundleName</key><string>BackplaneGPUI</string><key>CFBundlePackageType</key><string>APPL</string><key>NSHighResolutionCapable</key><true/></dict></plist>
P
codesign --force --sign - $A >/dev/null 2>&1 || true
rm -f logs/$L.out logs/$L.err
open -n -W --stdout $PWD/logs/$L.out --stderr $PWD/logs/$L.err $A --args "$@"
R
