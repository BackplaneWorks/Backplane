#!/bin/sh
# Build into dist/:
#   dist/backplane-serve  the hub and web server (src/server/main.bend)
#   dist/backplane        the launcher: the desktop app when installed, else
#                         the hub (deploy/backplane-launch.sh)
#   dist/web/             the web client (for other devices)
#   dist/backplane-browser, dist/backplane-step2glb, dist/backplane-voice, dist/backplane-kipc
#                         host helpers
#
# The hub is JS on Bun's runtime (scripts/build-js.sh, docs/js-service.md):
# bend emits it once (kept by a hash of what it reads) and `bun build
# --compile` makes one file; BACKPLANE_JS_TARGET cross-builds another
# target's from here.
set -eu
cd "$(dirname "$0")/.."
. scripts/bendok.sh
export PATH="$HOME/.bun/bin:$PATH"
command -v bun >/dev/null 2>&1 || { echo "build.sh: bun is needed" >&2; exit 1; }
# CI's "laws" job (scripts/check.sh) has already checked every file, these
# roots included: BACKPLANE_SKIP_CHECK=1 does not do it again
[ "${BACKPLANE_SKIP_CHECK:-}" = 1 ] || for f in src/web/app.bend src/server/main.bend; do
  bend_check "$f" || exit 1
done
# a release build stamps its version (BACKPLANE_VERSION=v1.2.3) into a
# copy of the emitted JS (scripts/build-js.sh, BACKPLANE_VERSION_STAMP)
if [ -n "${BACKPLANE_VERSION:-}" ]; then
  export BACKPLANE_VERSION_STAMP="${BACKPLANE_VERSION#v}"
fi
rm -rf dist
mkdir -p dist build
# the web client and the hub are emitted side by side
(
  bend src/web/index.html -o dist/web
  python3 scripts/js-unmarshal.py ../core/client.Ui dist/web/chunk-*.js
  python3 scripts/js-native.py dist/web/chunk-*.js
  cp src/web/sw.js dist/web/sw.js
  # precompressed copies: the server sends these to browsers that take gzip
  for f in dist/web/*.js dist/web/*.css dist/web/*.html; do gzip -9 -k -n -f "$f"; done
) > build/web-dist.log 2>&1 &
webpid=$!
hubrc=0
scripts/build-js.sh src/server/main.bend dist/backplane-serve || hubrc=$?
webrc=0
wait "$webpid" || webrc=$?
[ "$webrc" = 0 ] || { cat build/web-dist.log >&2; echo "build.sh: the web client did not build" >&2; exit 1; }
[ "$hubrc" = 0 ] || { echo "build.sh: the hub did not build" >&2; exit 1; }
# Bun's and Bend's notices, which the hub ships with (scripts/js-licenses.sh)
scripts/js-licenses.sh dist/licenses
# KiStack's skills, pinned and checked (deploy/kistack.lock)
scripts/build-kistack.sh dist/kistack
# The desktop UI is the Electron app over the hub; the package carries its
# launcher.
cp deploy/backplane-launch.sh dist/backplane
chmod 755 dist/backplane
# the host helpers (Bun too): the agents' browser, STEP -> GLB for the 3D
# viewer, speech to text for dictation and KiCad's IPC
scripts/build-browser.sh
scripts/build-step2glb.sh
scripts/build-voice.sh
scripts/build-kipc.sh
if [ "${BACKPLANE_CEF_RELEASE:-0}" = 1 ]; then
  scripts/stage-cef-release.sh
fi
ls -la dist
