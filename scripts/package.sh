#!/bin/sh
# Package dist/ into dist/backplane-<version>-<os>-<arch>.tar.gz:
#   backplane-<os>-<arch>/{backplane (launcher), backplane-serve, web/, kistack/, helpers, licenses/,
#   backplane.desktop, icon/}
#   scripts/package.sh v0.2.0 linux-x64
set -eu
cd "$(dirname "$0")/.."
ver=$1 plat=$2
dir="dist/pkg/backplane-$plat"
rm -rf dist/pkg && mkdir -p "$dir"
cp dist/backplane-serve LICENSE "$dir/"
[ -f dist/backplane ] && cp dist/backplane "$dir/"
cp -R dist/web "$dir/web"
cp -R dist/kistack "$dir/kistack"
for h in backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc; do
  [ -f "dist/$h" ] && cp "dist/$h" "$dir/"
done
[ -d dist/licenses ] && cp -R dist/licenses "$dir/licenses"
cp scripts/install-release.sh "$dir/install-release.sh"
# the desktop file and icons, for the Linux packages (scripts/package-linux.sh)
cp deploy/backplane.desktop "$dir/"
mkdir -p "$dir/icon"
cp assets/icon/backplane.svg assets/icon/backplane-256.png "$dir/icon/"
cp assets/icon/backplane-desktop.svg assets/icon/backplane-desktop-256.png "$dir/icon/"
tar -czf "dist/backplane-$ver-$plat.tar.gz" -C dist/pkg "backplane-$plat"
ls -la "dist/backplane-$ver-$plat.tar.gz"
