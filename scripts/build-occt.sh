#!/bin/sh
# scripts/build-occt.sh [PREFIX]: OpenCascade (OCCT) built from its pinned source as static libraries, with only the toolkits
# backplane-step2glb uses (native/step2glb/CMakeLists.txt) and none of its optional products (no OpenGL, Tk, Freetype, X11, TBB), into
# PREFIX (default ~/.cache/backplane/occt-<version>-static-<os>-<arch>). Done once a prefix: a second run says where it is and stops.
# Prints the folder of OpenCASCADEConfig.cmake last, for scripts/build-step2glb.sh. Needs cmake and a C++17 compiler (MSVC on Windows).
# OCCT is LGPL-2.1 with its exception (deploy/licenses/step2glb): a static build is shipped with its notices and its source's address.
set -eu
VER=7.9.2; TAG=V7_9_2
SHA=3cd080d3fc33ba0c6c157e110afe3e015859524c4694dbb09812ec9d61595639   # github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V7_9_2.tar.gz
OS=$(uname -s | tr 'A-Z' 'a-z'); case "$OS" in mingw*|msys*|cygwin*) OS=windows;; esac
ARCH=$(uname -m)
CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/backplane
PREFIX=${1:-$CACHE/occt-$VER-static-$OS-$ARCH}
cfg() { for d in "$PREFIX/lib/cmake/opencascade" "$PREFIX/cmake" "$PREFIX/lib/cmake"; do [ -f "$d/OpenCASCADEConfig.cmake" ] && { echo "$d"; return 0; }; done; return 1; }
if [ -f "$PREFIX/.done" ] && cfg > /dev/null; then echo "occt: $VER already built in $PREFIX" >&2; cfg; exit 0; fi
TMP=${BP_TMP:-${TMPDIR:-/tmp}}/occt-build-$$
mkdir -p "$TMP" "$CACHE"
trap 'rm -rf "$TMP"' EXIT
SRC=$CACHE/occt-$VER.tar.gz
if [ ! -f "$SRC" ] || ! { sha256sum "$SRC" 2>/dev/null || shasum -a 256 "$SRC"; } | grep -q "^$SHA"; then
  curl -fsSL --retry 3 -o "$SRC.part" "https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/$TAG.tar.gz"
  { sha256sum "$SRC.part" 2>/dev/null || shasum -a 256 "$SRC.part"; } | grep -q "^$SHA" || { echo "occt: the source's sha256 is not the pinned one" >&2; rm -f "$SRC.part"; exit 1; }
  mv "$SRC.part" "$SRC"
fi
tar xzf "$SRC" -C "$TMP"
JOBS=${JOBS:-$( (nproc || sysctl -n hw.ncpu || echo 4) 2>/dev/null | head -1)}
# the toolkits step2glb calls: each brings its dependencies (OCCT's EXCTRACT_TOOLKIT_FULL_DEPS); every module is off otherwise
KITS="TKXCAF;TKDESTEP;TKDE;TKXSBase;TKVCAF;TKCAF;TKLCAF;TKCDF;TKV3d;TKService;TKMesh;TKShHealing;TKTopAlgo;TKGeomAlgo;TKBRep;TKGeomBase;TKG3d;TKG2d;TKMath;TKernel"
GEN=; [ "$OS" = windows ] || { command -v ninja > /dev/null 2>&1 && GEN="-G Ninja"; }
# shellcheck disable=SC2086
cmake -S "$TMP/OCCT-$(echo $TAG | sed 's/^V//')" -B "$TMP/build" $GEN \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_LIBRARY_TYPE=Static -DINSTALL_DIR="$PREFIX" -DCMAKE_INSTALL_PREFIX="$PREFIX" \
  -DBUILD_MODULE_FoundationClasses=OFF -DBUILD_MODULE_ModelingData=OFF -DBUILD_MODULE_ModelingAlgorithms=OFF -DBUILD_MODULE_Visualization=OFF \
  -DBUILD_MODULE_ApplicationFramework=OFF -DBUILD_MODULE_DataExchange=OFF -DBUILD_MODULE_DETools=OFF -DBUILD_MODULE_Draw=OFF \
  -DBUILD_ADDITIONAL_TOOLKITS="$KITS" -DBUILD_DOC_Overview=OFF -DBUILD_Inspector=OFF -DBUILD_SAMPLES_QT=OFF -DBUILD_USE_PCH=OFF \
  -DUSE_FREETYPE=OFF -DUSE_OPENGL=OFF -DUSE_GLES2=OFF -DUSE_D3D=OFF -DUSE_TK=OFF -DUSE_XLIB=OFF -DUSE_TBB=OFF -DUSE_VTK=OFF \
  -DUSE_FREEIMAGE=OFF -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_OPENVR=OFF -DUSE_FFMPEG=OFF -DUSE_EIGEN=OFF \
  ${CMAKE_OSX_ARCHITECTURES:+-DCMAKE_OSX_ARCHITECTURES=$CMAKE_OSX_ARCHITECTURES} \
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON > "$TMP/configure.log" 2>&1 || { tail -40 "$TMP/configure.log" >&2; exit 1; }
cmake --build "$TMP/build" --config Release --parallel "$JOBS" > "$TMP/build.log" 2>&1 || { tail -60 "$TMP/build.log" >&2; exit 1; }
cmake --install "$TMP/build" --config Release > /dev/null
# its notices beside it, for the package (scripts/licenses.sh)
mkdir -p "$PREFIX/licenses"; cp "$TMP/OCCT-$(echo $TAG | sed 's/^V//')/LICENSE_LGPL_21.txt" "$TMP/OCCT-$(echo $TAG | sed 's/^V//')/OCCT_LGPL_EXCEPTION.txt" "$PREFIX/licenses/"
touch "$PREFIX/.done"
echo "occt: $VER built in $PREFIX" >&2
cfg
