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
rm -rf "$prefix/web.old"
[ ! -d "$prefix/web" ] || mv "$prefix/web" "$prefix/web.old"
mv "$src/web" "$prefix/web"
# Carry the bundled KiStack skills added by the security release.
if [ -d "$src/kistack" ]; then
  rm -rf "$prefix/kistack.old"
  [ ! -d "$prefix/kistack" ] || mv "$prefix/kistack" "$prefix/kistack.old"
  mv "$src/kistack" "$prefix/kistack"
fi
for file in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc LICENSE; do
  if [ -e "$src/$file" ]; then mv -f "$src/$file" "$prefix/$file"; fi
done
# An older install kept its browser helper behind links into an embedded
# browser runtime; the helper above replaced the links, so the rest goes.
[ -L "$prefix/backplane-browser" ] || rm -rf "$prefix/cef" "$prefix/browser-current" "$prefix/browser-previous" "$prefix/browser-runtimes"
if [ -d "$src/licenses" ]; then
  rm -rf "$prefix/licenses"
  mv "$src/licenses" "$prefix/licenses"
fi
# without a launcher in the release, `backplane` (and the links to it) must
# not stay the old window binary or point at nothing: it runs the hub
if [ "$launcher" = 0 ]; then
  ln -sfn backplane-serve "$prefix/backplane.new" && mv -f "$prefix/backplane.new" "$prefix/backplane"
fi
