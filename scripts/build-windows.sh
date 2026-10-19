#!/bin/sh
# Compile platform-neutral Bend C to a native Windows executable in Cygwin.
# The pinned Bend compiler may emit C on Linux/macOS; no Linux runtime is
# needed by the executable. Install Cygwin clang (19+), python3 and libraries
# listed in docs/windows.md, then: scripts/build-windows.sh app.c out.exe
set -eu
case $(uname -s) in CYGWIN_NT-*) ;; *) echo 'build-windows: run inside Windows Cygwin' >&2; exit 2;; esac
[ $# = 2 ] || { echo 'usage: build-windows.sh INPUT.c OUTPUT.exe' >&2; exit 2; }
input=$(realpath "$1")
out=$(realpath -m "$2")
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
work="${out%.exe}.cc"
mkdir -p "$work"
BP_WIN_TELEMETRY_HEADER="$work/bp_telemetry.h"
trap 'rm -f "$BP_WIN_TELEMETRY_HEADER"' EXIT
python3 "$root/scripts/telemetry-header.py" "$BP_WIN_TELEMETRY_HEADER"
export BP_WIN_TELEMETRY_HEADER
cc=${CC:-clang}
jobs=${BACKPLANE_JOBS:-1}
units=${BACKPLANE_UNITS:-16}
python3 "$root/scripts/windows-runtime.py" "$input" "$work/runtime.c"
python3 "$root/scripts/cc-split.py" "$work/runtime.c" "$work" "$units"
# One compiler by default keeps Windows and the host desktop responsive.
export BP_WIN_CC="$cc" BP_WIN_FLAGS="${BACKPLANE_CFLAGS:--O2}" BP_WIN_INCLUDE="$root/src/server/effects"
find "$work" -maxdepth 1 -name 'u*.c' -print | sort | xargs -P "$jobs" -I{} sh -c \
  '"$BP_WIN_CC" -std=c11 -D___wchar_t=__WCHAR_TYPE__ -I"$BP_WIN_INCLUDE" -include "$BP_WIN_TELEMETRY_HEADER" $BP_WIN_FLAGS -Wno-unknown-attributes -c "$1" -o "${1%.c}.o" || exit 255' _ {}
"$cc" "$work"/u*.o -o "$out" -lpthread -lm -luser32 -lgdi32 -lshell32 -limm32 -lbcrypt
file "$out"
