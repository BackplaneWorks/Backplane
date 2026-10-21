#!/bin/sh
# Shared public installer/updater path. No restart or user-data changes.
set -eu
src=$1 prefix=$2
# backplane-serve is required; `backplane` (the launcher,
# deploy/backplane-launch.sh) may be left out
for file in backplane-serve; do
  [ -f "$src/$file" ] || { echo "release missing $file" >&2; exit 1; }
done
launcher=0
[ ! -e "$src/backplane" ] || launcher=1
[ -d "$src/web" ] || { echo 'release missing web' >&2; exit 1; }
mkdir -p "$prefix"
if [ -d "$src/browser-runtime" ]; then
  command -v python3 >/dev/null 2>&1 || { echo 'CEF installation/update needs python3; no files replaced' >&2; exit 1; }
  [ -f "$src/cef-runtime.py" ] || { echo 'release missing CEF verifier' >&2; exit 1; }
  python3 "$src/cef-runtime.py" install-release "$src" "$prefix"
elif [ -e "$prefix/browser-current" ] || [ -L "$prefix/browser-current" ]; then
  echo 'release lacks the installed CEF pair; no files replaced' >&2; exit 1
fi
rm -rf "$prefix/web.old"
[ ! -d "$prefix/web" ] || mv "$prefix/web" "$prefix/web.old"
mv "$src/web" "$prefix/web"
# Carry the bundled KiStack skills added by the security release. The CEF
# preflight above must succeed before any installed skill tree is replaced.
if [ -d "$src/kistack" ]; then
  rm -rf "$prefix/kistack.old"
  [ ! -d "$prefix/kistack" ] || mv "$prefix/kistack" "$prefix/kistack.old"
  mv "$src/kistack" "$prefix/kistack"
fi
for file in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc LICENSE; do
  if [ -e "$src/$file" ]; then mv -f "$src/$file" "$prefix/$file"; fi
done
if [ -d "$src/licenses" ]; then
  rm -rf "$prefix/licenses"
  mv "$src/licenses" "$prefix/licenses"
fi
# without a launcher in the release, `backplane` (and the links to it) must
# not stay the old window binary or point at nothing: it runs the hub
if [ "$launcher" = 0 ]; then
  ln -sfn backplane-serve "$prefix/backplane.new" && mv -f "$prefix/backplane.new" "$prefix/backplane"
fi
