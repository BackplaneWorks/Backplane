#!/bin/bash
# windows-check.sh [host|gpui|all]: `cargo check --target x86_64-pc-windows-gnu --all-targets` of the hub and/or the client on this Linux machine,
# windows-check.sh --build [DIR]: Windows programs made here (debug): backplane-hub.exe, bp-sh.exe, backplane-gpui.exe and llvm-mingw's libunwind.dll
#   in DIR (default build/windows), linked for x86_64-pc-windows-gnullvm (llvm-mingw has no libgcc, which the -gnu target links), to copy to a Windows machine.
# through scripts/cargo-q (build slots, sccache). C code in dependencies (ring, the client's) is compiled by llvm-mingw's clang, in user space:
#   WIN_LLVM_MINGW=<dir>   the unpacked llvm-mingw (github.com/mstorsjo/llvm-mingw, ucrt-ubuntu-*-x86_64), default the newest ~/.local/opt/llvm-mingw-*
# A check runs no build script that needs Windows: gpui compiles its HLSL shaders only on a Windows build host (and only in release),
# so a debug build made here compiles them when the window opens (D3DCompile); a release client must be built on Windows (CI, release.yml).
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
tc=${WIN_LLVM_MINGW:-$(ls -d "$HOME"/.local/opt/llvm-mingw-* 2>/dev/null | sort | tail -1)}
[ -x "$tc/bin/x86_64-w64-mingw32-clang" ] || { echo "windows-check: no llvm-mingw (set WIN_LLVM_MINGW, or unpack one into ~/.local/opt)" >&2; exit 2; }
rustup target list --installed | grep -qx x86_64-pc-windows-gnu || { echo "windows-check: rustup target add x86_64-pc-windows-gnu" >&2; exit 2; }
export CC_x86_64_pc_windows_gnu=$tc/bin/x86_64-w64-mingw32-clang CXX_x86_64_pc_windows_gnu=$tc/bin/x86_64-w64-mingw32-clang++ AR_x86_64_pc_windows_gnu=$tc/bin/llvm-ar
if [ "${1:-}" = --build ]; then
  rustup target list --installed | grep -qx x86_64-pc-windows-gnullvm || { echo "windows-check: rustup target add x86_64-pc-windows-gnullvm" >&2; exit 2; }
  out=${2:-$HERE/build/windows}; mkdir -p "$out"; T=x86_64_pc_windows_gnullvm
  export CC_$T=$tc/bin/x86_64-w64-mingw32-clang CXX_$T=$tc/bin/x86_64-w64-mingw32-clang++ AR_$T=$tc/bin/llvm-ar CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER=$tc/bin/x86_64-w64-mingw32-clang
  tgt=${CARGO_TARGET_DIR:-${BP_TMP:-$HOME/.cache/bp-tmp}/target-wingnullvm}; export CARGO_TARGET_DIR=$tgt   # never under /tmp (cargo-q refuses it)
  (cd "$HERE/native/host" && "$HERE/scripts/cargo-q" build --offline --target x86_64-pc-windows-gnullvm --bin backplane-hub --bin bp-sh)
  (cd "$HERE/native/gpui" && "$HERE/scripts/cargo-q" build --offline --target x86_64-pc-windows-gnullvm)
  for f in backplane-hub.exe bp-sh.exe backplane-gpui.exe; do cp "$tgt/x86_64-pc-windows-gnullvm/debug/$f" "$out/"; done
  cp "$tc/x86_64-w64-mingw32/bin/libunwind.dll" "$out/"
  echo "windows-check: $out holds $(cd "$out" && ls | tr '\n' ' ')"
  exit 0
fi
# its own target dir: a Windows check must not evict the Linux build's artifacts
rc=0
for c in ${1:-all}; do case $c in all) set -- host gpui;; *) set -- "$c";; esac
  for k in "$@"; do
    echo "== $k"
    (cd "$HERE/native/$k" && CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HERE/native/$k/target} "$HERE/scripts/cargo-q" check --offline --target x86_64-pc-windows-gnu --all-targets) || rc=1
  done
done
exit $rc
