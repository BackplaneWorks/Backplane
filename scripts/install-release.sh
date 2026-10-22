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
fi
# A release without CEF (0.12.1 on) leaves an installed CEF pair alone: the
# hub no longer uses it, and refusing would strand every install that had one.
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
# The desktop app (desktop/, from 0.12.1 on). A 0.12.0 desktop app refuses a
# hub of any other version, so an update must bring the app with the hub: it
# replaces an installed desktop/, and install.sh (BACKPLANE_INSTALL_DESKTOP=1)
# adds one; a hub-only install (BACKPLANE_NO_DESKTOP_APP=1, or no desktop/
# installed) stays hub-only. A chrome-sandbox the person made root-owned and
# setuid is kept when the new one is the same file, so the sandbox needs no
# sudo again.
if [ -d "$src/desktop" ] && [ -z "${BACKPLANE_NO_DESKTOP_APP:-}" ] &&
   { [ -d "$prefix/desktop" ] || [ -n "${BACKPLANE_INSTALL_DESKTOP:-}" ]; }; then
  old=$prefix/desktop/runtime/chrome-sandbox new=$src/desktop/runtime/chrome-sandbox
  if [ -f "$old" ] && [ -f "$new" ] && [ -n "$(find "$old" -user 0 -perm -4000 2>/dev/null)" ] && cmp -s "$old" "$new"; then
    mv -f "$old" "$new"
  fi
  rm -rf "$prefix/desktop.old"
  [ ! -d "$prefix/desktop" ] || mv "$prefix/desktop" "$prefix/desktop.old"
  mv "$src/desktop" "$prefix/desktop"
fi
# without a launcher in the release, `backplane` (and the links to it) must
# not stay the old window binary or point at nothing: it runs the hub
if [ "$launcher" = 0 ]; then
  ln -sfn backplane-serve "$prefix/backplane.new" && mv -f "$prefix/backplane.new" "$prefix/backplane"
fi
