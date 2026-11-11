#!/bin/sh
# scripts/build-step2glb.sh [OUT]: backplane-step2glb, the STEP mesher (native/step2glb: OpenCascade, natively; no Bun, no FreeCAD), built
# with CMake into OUT (default dist/backplane-step2glb, .exe on Windows). Its OpenCascade, in order:
#   OCCT_DIR=<folder of OpenCASCADEConfig.cmake>;
#   STATIC=1 (a release): scripts/build-occt.sh's pinned static build, so the program carries all of it, and its notices go to
#     <OUT's folder>/licenses/step2glb;
#   else the system's (a dev build: Debian/Ubuntu libocct-*-dev, Homebrew's opencascade).
# A build of the same sources against the same OpenCascade is kept (~/.cache/backplane/step2glb-<key>) and only copied again.
set -eu
cd "$(dirname "$0")/.."
ROOT=$PWD
EXE=; case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE=.exe;; esac
OUT=${1:-dist/backplane-step2glb$EXE}
CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/backplane
sha() { sha256sum 2>/dev/null || shasum -a 256; }
DIR=${OCCT_DIR:-}
if [ -z "$DIR" ] && [ "${STATIC:-0}" = 1 ]; then DIR=$(sh scripts/build-occt.sh | tail -1); fi
if [ -z "$DIR" ]; then
  for d in /usr/lib/*/cmake/opencascade /usr/lib/cmake/opencascade /usr/lib64/cmake/opencascade /opt/homebrew/lib/cmake/opencascade /usr/local/lib/cmake/opencascade; do
    [ -f "$d/OpenCASCADEConfig.cmake" ] && { DIR=$d; break; }
  done
fi
[ -n "$DIR" ] && [ -f "$DIR/OpenCASCADEConfig.cmake" ] || { echo "build-step2glb: no OpenCascade (set OCCT_DIR, STATIC=1, or install libocct-data-exchange-dev / brew opencascade)" >&2; exit 1; }
KEY=$( { cat native/step2glb/step2glb.cpp native/step2glb/CMakeLists.txt; echo "$DIR"; cat "$DIR/OpenCASCADEConfigVersion.cmake" 2>/dev/null; } | sha | cut -c1-16)
B=$CACHE/step2glb-$KEY
BIN=$B/out/backplane-step2glb$EXE
if [ ! -x "$BIN" ]; then
  rm -rf "$B"; mkdir -p "$B"
  GEN=; [ -z "$EXE" ] && command -v ninja > /dev/null 2>&1 && GEN="-G Ninja"
  # shellcheck disable=SC2086
  cmake -S native/step2glb -B "$B/build" $GEN -DCMAKE_BUILD_TYPE=Release -DOpenCASCADE_DIR="$DIR" -DCMAKE_INSTALL_PREFIX="$B/out" \
    ${STATIC:+-DOCCT_STATIC=$([ "$STATIC" = 1 ] && echo ON || echo OFF)} ${CMAKE_OSX_ARCHITECTURES:+-DCMAKE_OSX_ARCHITECTURES=$CMAKE_OSX_ARCHITECTURES} > "$B/configure.log" 2>&1 \
    || { tail -30 "$B/configure.log" >&2; exit 1; }
  cmake --build "$B/build" --config Release --parallel > "$B/build.log" 2>&1 || { tail -40 "$B/build.log" >&2; exit 1; }
  cmake --install "$B/build" --config Release > /dev/null
  # a release's program without its symbols (37 MB to 30 MB on Linux); MSVC keeps them in a .pdb already
  if [ "${STATIC:-0}" = 1 ] && [ -z "$EXE" ]; then case "$(uname -s)" in Darwin) strip -x "$BIN";; *) strip --strip-unneeded "$BIN";; esac; fi
  # the kept builds: the newest four
  ls -dt "$CACHE"/step2glb-* 2>/dev/null | tail -n +5 | while read -r d; do rm -rf "$d"; done
fi
mkdir -p "$(dirname "$OUT")"
cp "$BIN" "$OUT.tmp" && mv "$OUT.tmp" "$OUT"
# a static OpenCascade's notices (LGPL-2.1 and OCCT's exception) ship with the program
L=$(dirname "$DIR"); while [ "$L" != / ] && [ ! -d "$L/licenses" ]; do L=$(dirname "$L"); done
if [ -d "$L/licenses" ] && [ -f "$L/licenses/LICENSE_LGPL_21.txt" ]; then mkdir -p "$(dirname "$OUT")/licenses/step2glb"; cp "$L/licenses/"*.txt "$(dirname "$OUT")/licenses/step2glb/"; fi
# it runs: a bad argument answers 2, a missing STEP 3
"$OUT" > /dev/null 2>&1 && rc=0 || rc=$?
[ "$rc" = 2 ] || { echo "build-step2glb: $OUT does not run (exit $rc)" >&2; exit 1; }
echo "$OUT"
