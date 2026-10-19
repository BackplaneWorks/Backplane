#!/bin/sh
# Package dist/ into dist/backplane-<version>-<os>-<arch>.tar.gz:
#   backplane-<os>-<arch>/{backplane, backplane-serve, web/, helpers, licenses/,
#   backplane.desktop, icon/}
#   scripts/package.sh v0.2.0 linux-x64
set -eu
cd "$(dirname "$0")/.."
ver=$1 plat=$2
dir="dist/pkg/backplane-$plat"
rm -rf dist/pkg && mkdir -p "$dir"
cp dist/backplane dist/backplane-serve LICENSE "$dir/"
cp -R dist/web "$dir/web"
for h in backplane-browser backplane-step2glb backplane-voice backplane-kipc; do
  [ -f "dist/$h" ] && cp "dist/$h" "$dir/"
done
[ -d dist/licenses ] && cp -R dist/licenses "$dir/licenses"
# the desktop file and icons, for the Linux packages (scripts/package-linux.sh)
cp deploy/backplane.desktop "$dir/"
mkdir -p "$dir/icon"
cp assets/icon/backplane-desktop.svg "$dir/icon/backplane.svg"
cp assets/icon/backplane-256.png "$dir/icon/"
tar -czf "dist/backplane-$ver-$plat.tar.gz" -C dist/pkg "backplane-$plat"
ls -la "dist/backplane-$ver-$plat.tar.gz"
