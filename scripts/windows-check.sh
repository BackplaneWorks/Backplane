#!/bin/bash
# windows-check.sh [host|gpui|all]: `cargo check --target x86_64-pc-windows-gnu --all-targets` of the hub and/or the client on this Linux machine,
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
# its own target dir: a Windows check must not evict the Linux build's artifacts
rc=0
for c in ${1:-all}; do case $c in all) set -- host gpui;; *) set -- "$c";; esac
  for k in "$@"; do
    echo "== $k"
    (cd "$HERE/native/$k" && CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HERE/native/$k/target} "$HERE/scripts/cargo-q" check --offline --target x86_64-pc-windows-gnu --all-targets) || rc=1
  done
done
exit $rc
