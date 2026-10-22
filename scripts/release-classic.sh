#!/bin/sh
# The classic packages and install.sh of a release, from what it already
# built (nothing is compiled or relabelled here):
#
#   scripts/release-classic.sh v0.12.0 OUT \
#     linux-x64 SERVICE_PAYLOAD DESKTOP_ARCHIVE [linux-arm64 ... darwin-arm64 ...]
#
# SERVICE_PAYLOAD is the accepted service component's payload/ (its
# backplane-serve, web/, licences and receipts, as built); DESKTOP_ARCHIVE is
# the release's backplane-<ver>-<plat>.tar.gz (the desktop app). Each
# platform gets OUT/backplane-<ver>-classic-<plat>.tar.gz: the payload, the
# `backplane` launcher, install-release.sh, the menu entry and icons, under
# backplane-<plat>/. That is the layout install.sh unpacks and the one every
# 0.11 updater swaps in: it takes the first asset whose name ends in
# <plat>.tar.gz, and GitHub lists assets by name, so -classic- (c) comes
# before the desktop archive's <plat> (d, l). OUT/install.sh carries the
# sha256 of both archives for each platform. For linux-x64 the classic
# package is also written as backplane-<ver>-linux-x64-cef.tar.gz, byte for
# byte, for the v0.12.0 updater.
set -eu
[ $# -ge 5 ] && [ $(( ($# - 2) % 3 )) -eq 0 ] || { sed -n '2,20p' "$0"; exit 2; }
abs() { case $1 in /*) printf '%s\n' "$1" ;; *) printf '%s/%s\n' "$PWD" "$1" ;; esac; }
ver=$1 out=$(abs "$2")
shift 2
# inputs as absolute paths, before moving to the checkout
n=$#
while [ $n -gt 0 ]; do
  set -- "$@" "$1" "$(abs "$2")" "$(abs "$3")"
  shift 3
  n=$((n - 3))
done
cd "$(dirname "$0")/.."
case $ver in v[0-9]*) ;; *) echo "release-classic: version must be vX.Y.Z" >&2; exit 2 ;; esac
mkdir -p "$out"
out=$(cd "$out" && pwd)
sum() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }
seds=
while [ $# -gt 0 ]; do
  plat=$1 payload=$2 desktop=$3
  shift 3
  case $plat in linux-x64|linux-arm64|darwin-arm64) ;; *) echo "release-classic: no classic package for $plat" >&2; exit 2 ;; esac
  [ -x "$payload/backplane-serve" ] && [ -f "$payload/web/index.html" ] || { echo "release-classic: $payload is no service payload" >&2; exit 1; }
  [ "$(basename "$desktop")" = "backplane-$ver-$plat.tar.gz" ] || { echo "release-classic: $desktop is not backplane-$ver-$plat.tar.gz" >&2; exit 1; }
  tar -tzf "$desktop" "backplane-$plat/" >/dev/null 2>&1 || tar -tzf "$desktop" | grep -q "^backplane-$plat/" || { echo "release-classic: $desktop has no backplane-$plat/" >&2; exit 1; }
  # the service must say this version (it runs on the matching host only)
  stage=$(mktemp -d)
  dir=$stage/backplane-$plat
  cp -R "$payload" "$dir"
  cp deploy/backplane-launch.sh "$dir/backplane"
  chmod 755 "$dir/backplane"
  cp scripts/install-release.sh "$dir/install-release.sh"
  [ -f "$dir/LICENSE" ] || cp "$dir/LICENSE.Backplane" "$dir/LICENSE"
  cp deploy/backplane.desktop "$dir/"
  mkdir -p "$dir/icon"
  cp assets/icon/backplane-desktop.svg assets/icon/backplane-desktop-256.png "$dir/icon/"
  name=backplane-$ver-classic-$plat.tar.gz
  # fixed order, owner and times: the same inputs give the same bytes
  (cd "$stage" && find "backplane-$plat" -print | LC_ALL=C sort > list &&
    tar --no-recursion --owner=0 --group=0 --numeric-owner --mtime=@0 -cf - -T list | gzip -9 -n > "$out/$name")
  rm -rf "$stage"
  # legacy name the v0.12.0 updater looks for (its suffix linux-x64-cef.tar.gz)
  if [ "$plat" = linux-x64 ]; then
    cp "$out/$name" "$out/backplane-$ver-linux-x64-cef.tar.gz"
    echo "$out/backplane-$ver-linux-x64-cef.tar.gz"
  fi
  key=$(printf '%s' "$plat" | tr 'a-z-' 'A-Z_')
  seds="$seds -e s/@SHA_$key@/$(sum "$out/$name")/ -e s/@DESKTOP_SHA_$key@/$(sum "$desktop")/"
  echo "$out/$name"
done
# a platform this release has no build for keeps an empty sum: install.sh
# then says so instead of downloading
sed -e "s/@VERSION@/$ver/" $seds -e 's/@SHA_[A-Z0-9_]*@//' -e 's/@DESKTOP_SHA_[A-Z0-9_]*@//' deploy/install.sh.in > "$out/install.sh"
chmod 755 "$out/install.sh"
echo "$out/install.sh"
