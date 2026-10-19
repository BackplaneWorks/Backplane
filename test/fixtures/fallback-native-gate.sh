#!/bin/sh
# Native integration artifacts. Use an admitted local job, or a provisioned
# disposable Boat sandbox; no credentials or live hub state are copied.
set -eu
export CC=${CC:-clang} BACKPLANE_LOW_MEM=1 BACKPLANE_GOOGLE_BAKE=0
if ! command -v "$CC" >/dev/null || [ ! -e /usr/include/X11/Xlib.h ]; then
  echo "Native build dependencies are missing; provision a compiler and X11 headers before running this gate." >&2
  exit 78
fi
. scripts/bendok.sh
bend_check src/app/main.bend
bend_check test/native/ui_snap.bend
bend_check src/web/app.bend
bend_check test/wire/wire.bend
scripts/build-app.sh src/app/main.bend build/fallback-app
scripts/build-app.sh test/native/ui_snap.bend build/fallback-ui-snap
bend src/web/index.html -o build/fallback-web
python3 scripts/js-unmarshal.py ../core/client.Ui build/fallback-web/chunk-*.js
python3 scripts/js-native.py build/fallback-web/chunk-*.js
cp src/web/sw.js build/fallback-web/sw.js
bend test/wire/index.html -o build/fallback-wire
sha256sum build/fallback-app build/fallback-ui-snap > build/fallback-native.sha256
