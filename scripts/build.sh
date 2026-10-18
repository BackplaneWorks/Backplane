#!/bin/sh
# Build into dist/:
#   dist/backplane        the app: native window + hub + web server
#   dist/backplane-serve  hub + web server only (no X11 needed)
#   dist/web/             the web client (for other devices)
#   dist/backplane-browser, dist/backplane-step2glb, dist/backplane-voice, dist/backplane-kipc
#                         host helpers (with bun)
# X11 headers: /usr/include (libx11-dev), or BACKPLANE_X11=<prefix> holding
# include/ and lib/libX11.so (see AGENTS.md).
# System fonts (src/gfx/effects/font.c) need no headers and no link flags:
# libfreetype.so.6 and libfontconfig.so.1 are dlopen'd at run time and the
# few FreeType structs read are declared in font.c; without them text falls
# back to the Spleen bitmap font. To re-check those struct layouts against
# real headers without root:
#   apt-get download libfreetype-dev && dpkg -x libfreetype-dev_*.deb ~/.local/ft
#   (headers in ~/.local/ft/usr/include/freetype2; compare offsetof values)
#
# A macOS runner's 7 GB is too little for bend's emit, so releases split
# the build (.github/workflows/release.yml): `scripts/build.sh emit` on
# Linux writes what is the same everywhere (the web client, the server's C
# and test/wire for smoke.sh), and `BACKPLANE_PREBUILT=1 scripts/build.sh`
# on the Mac compiles it.
set -eu
cd "$(dirname "$0")/.."
. scripts/bendok.sh
mode=${1:-all}
pre=${BACKPLANE_PREBUILT:-}
# CI's "laws" job (scripts/check.sh) has already checked every file, these
# three roots included: BACKPLANE_SKIP_CHECK=1 does not do it again
[ "$pre" = 1 ] || [ "${BACKPLANE_SKIP_CHECK:-}" = 1 ] || for f in src/web/app.bend src/server/main.bend src/app/main.bend; do
  bend_check "$f" || exit 1
done
if [ -n "${BACKPLANE_X11:-}" ]; then
  export CPATH="$BACKPLANE_X11/include${CPATH:+:$CPATH}"
  export LIBRARY_PATH="$BACKPLANE_X11/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
fi
# a release build stamps its version (BACKPLANE_VERSION=v1.2.3) into the
# native binaries' emitted C (scripts/build-app.sh, BACKPLANE_VERSION_STAMP)
if [ -n "${BACKPLANE_VERSION:-}" ]; then
  export BACKPLANE_VERSION_STAMP="${BACKPLANE_VERSION#v}"
fi
if [ "$pre" = 1 ]; then
  [ -f dist/web/index.html ] || { echo "BACKPLANE_PREBUILT: no dist/web (scripts/build.sh emit)"; exit 1; }
  find dist -mindepth 1 -maxdepth 1 ! -name web -exec rm -rf {} +
else
  rm -rf dist
  mkdir -p dist
  bend src/web/index.html -o dist/web
  python3 scripts/js-unmarshal.py ../core/client.Ui dist/web/chunk-*.js
  python3 scripts/js-native.py dist/web/chunk-*.js
  cp src/web/sw.js dist/web/sw.js
  # precompressed copies: the server sends these to browsers that take gzip
  for f in dist/web/*.js dist/web/*.css dist/web/*.html; do gzip -9 -k -n -f "$f"; done
fi
mkdir -p build
if [ "$mode" = emit ]; then
  BACKPLANE_EMIT_ONLY=1 scripts/build-app.sh src/server/main.bend dist/backplane-serve
  rm -rf build/wire
  bend test/wire/index.html -o build/wire > /dev/null
  exit 0
fi
# native binaries compile split in parallel, at low priority
# (scripts/build-app.sh; BACKPLANE_JOBS caps the compiles)
scripts/build-app.sh src/server/main.bend dist/backplane-serve
# the window needs X11 (libX11.so.6, dlopen'd); a Mac has none, so there the
# app always runs as hub + web UI, the same program as backplane-serve. The
# app's C also takes far more memory than a macOS runner (7 GB) has; build
# it only where a window can open
if [ "$(uname -s)" = Darwin ]; then
  cp dist/backplane-serve dist/backplane
else
  scripts/build-app.sh src/app/main.bend dist/backplane
fi
cp dist/backplane build/backplane
# the host helpers (Bun): the agents' browser, STEP -> GLB for the 3D
# viewer and speech to text for dictation; the app runs without them, so a machine without bun skips them
if command -v bun >/dev/null 2>&1 || [ -x "$HOME/.bun/bin/bun" ]; then
  PATH="$HOME/.bun/bin:$PATH" scripts/build-browser.sh
  PATH="$HOME/.bun/bin:$PATH" scripts/build-step2glb.sh
  PATH="$HOME/.bun/bin:$PATH" scripts/build-voice.sh
  PATH="$HOME/.bun/bin:$PATH" scripts/build-kipc.sh
else
  echo "bun not found: skipping backplane-browser, backplane-step2glb, backplane-voice and backplane-kipc"
fi
ls -la dist
