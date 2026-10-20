#!/bin/sh
# Package dist/ into dist/backplane-<version>-<os>-<arch>.tar.gz:
#   backplane-<os>-<arch>/{backplane, backplane-serve, web/, kistack/, helpers, licenses/,
#   backplane.desktop, icon/}
#   scripts/package.sh v0.2.0 linux-x64
set -eu
cd "$(dirname "$0")/.."
ver=$1 plat=$2
archive_plat=$plat
if [ "$plat" = linux-x64 ]; then
  # Old updaters cannot install a pair; a new suffix makes them refuse safely.
  python3 scripts/cef-runtime.py check-release dist >/dev/null
  archive_plat=linux-x64-cef
elif [ -d dist/browser-runtime ]; then
  echo "CEF payload is not supported for $plat" >&2; exit 1
fi
dir="dist/pkg/backplane-$plat"
rm -rf dist/pkg && mkdir -p "$dir"
cp dist/backplane dist/backplane-serve LICENSE "$dir/"
cp -R dist/web "$dir/web"
cp -R dist/kistack "$dir/kistack"
for h in backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc; do
  [ -f "dist/$h" ] && cp "dist/$h" "$dir/"
done
[ -d dist/licenses ] && cp -R dist/licenses "$dir/licenses"
cp scripts/install-release.sh "$dir/install-release.sh"
if [ "$plat" = linux-x64 ]; then
  cp -R dist/browser-runtime "$dir/browser-runtime"
  cp scripts/cef-runtime.py "$dir/cef-runtime.py"
  ln -s browser-runtime/cef "$dir/cef"
fi
# the desktop file and icons, for the Linux packages (scripts/package-linux.sh)
cp deploy/backplane.desktop "$dir/"
mkdir -p "$dir/icon"
cp assets/icon/backplane.svg assets/icon/backplane-256.png "$dir/icon/"
cp assets/icon/backplane-desktop.svg assets/icon/backplane-desktop-256.png "$dir/icon/"
tar -czf "dist/backplane-$ver-$archive_plat.tar.gz" -C dist/pkg "backplane-$plat"
ls -la "dist/backplane-$ver-$archive_plat.tar.gz"
