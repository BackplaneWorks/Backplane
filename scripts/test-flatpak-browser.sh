#!/bin/bash
# Requires an installed bundle, Freedesktop SDK 25.08, Bun, Xvfb, cc and X11 headers.
# Run under dbus-run-session. All displays, caches and profiles belong to this run.
set -euo pipefail
cd "$(dirname "$0")/.."
app=io.github.i2cjak.Backplane
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
export BACKPLANE_CEF="$PWD/build/cef/Release/backplane-cef"
export BACKPLANE_BROWSER="$scratch/browser"
export BACKPLANE_TEST_CACHE="$scratch/cache"
cat > "$BACKPLANE_BROWSER" <<'SH'
#!/bin/sh
exec flatpak run --nofilesystem=host --filesystem="$PWD:ro" --env="BACKPLANE_CEF=$BACKPLANE_CEF" --env="BACKPLANE_TEST_CACHE=$BACKPLANE_TEST_CACHE" --command=sh io.github.i2cjak.Backplane -c 'export XDG_CACHE_HOME="$BACKPLANE_TEST_CACHE"; exec /app/lib/backplane/backplane-browser "$@"' sh "$@"
SH
chmod +x "$BACKPLANE_BROWSER"
# Flatpak sets XDG_CACHE_HOME after applying --env, so isolate it in the shell.
flatpak run --nofilesystem=host --command=sh "$app" -c 'test ! -e /opt/google/chrome/chrome'
# Exercise first-use Chrome download, owner isolation, input, uploads/downloads,
# JPEG/RGB frames, screenshots, PDF and profile persistence in the actual app.
timeout 240 bun tools/browser/test/pages.ts
find "$BACKPLANE_TEST_CACHE/backplane/chrome" -name chrome -type f | grep -q .
# Build only the small host against prebuilt CEF, using the app's runtime ABI.
flatpak run --share=network --filesystem=host --filesystem=/tmp --command=sh org.freedesktop.Sdk//25.08 -c 'CC=gcc CXX=g++ scripts/build-cef.sh'
timeout 240 bun test/tools/flatpak_browser.ts
cat > "$scratch/app" <<'SH'
#!/bin/sh
exec flatpak run --nofilesystem=host --filesystem="$PWD:ro" --env="BACKPLANE_CEF=$BACKPLANE_CEF" --env="BACKPLANE_TEST_CACHE=$BACKPLANE_TEST_CACHE" --command=sh io.github.i2cjak.Backplane -c 'export XDG_CACHE_HOME="$BACKPLANE_TEST_CACHE"; exec /app/bin/backplane "$@"' sh "$@"
SH
chmod +x "$scratch/app"
bend test/wire/index.html -o "$scratch/wire" >/dev/null
export CEF_SHOTS="$PWD/build/flatpak-browser-shots" CEF_LOGS="$PWD/build/flatpak-browser-logs"
timeout 300 bun test/tools/cef_window.ts "$scratch/app" "$scratch/wire"
