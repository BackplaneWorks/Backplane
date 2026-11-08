#!/bin/bash
# windows-check.sh [host|gpui|all]: `cargo check --target x86_64-pc-windows-gnu --all-targets` of the hub and/or the client on this Linux machine,
# windows-check.sh --xwin [DIR]: release-target programs (x86_64-pc-windows-msvc, cargo-xwin; PROFILE, default fast) of the hub, guard, bp-sh, client and voice helper in DIR
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
# the client's manifest (vendored gpui embeds it for a Windows target built here): llvm-mingw's windres
export RC_x86_64_pc_windows_gnu=$tc/bin/x86_64-w64-mingw32-windres RC_x86_64_pc_windows_gnullvm=$tc/bin/x86_64-w64-mingw32-windres
if [ "${1:-}" = --xwin ]; then
  # x86_64-pc-windows-msvc through cargo-xwin (Microsoft's CRT and SDK, fetched once into ~/.cache/cargo-xwin): the release target.
  # The client is built with PROFILE (default fast: no debug assertions), so it draws with the shader bytes in native/gpui/vendor/gpui/windows-shaders.
  rustup target list --installed | grep -qx x86_64-pc-windows-msvc || { echo "windows-check: rustup target add x86_64-pc-windows-msvc" >&2; exit 2; }
  command -v cargo-xwin >/dev/null || { echo "windows-check: cargo install --locked cargo-xwin" >&2; exit 2; }
  llvm=${WIN_LLVM:-$(ls -d /usr/lib/llvm-*/bin 2>/dev/null | sort -V | tail -1)}
  [ -x "$llvm/clang-cl" ] && [ -x "$llvm/llvm-rc" ] || { echo "windows-check: no clang-cl/llvm-rc (set WIN_LLVM to an LLVM bin folder)" >&2; exit 2; }
  out=${2:-$HERE/build/windows}; mkdir -p "$out"; prof=${PROFILE:-fast}
  bin=${BP_TMP:-$HOME/.cache/bp-tmp}/xwin-bin; mkdir -p "$bin"
  command -v lld-link >/dev/null || [ -x "$llvm/lld-link" ] || ln -sf "$tc/bin/lld-link" "$bin/lld-link"   # only lld-link from llvm-mingw, not its clang
  export PATH=$llvm:$bin:$PATH XWIN_ACCEPT_LICENSE=1 RC_x86_64_pc_windows_msvc=$llvm/llvm-rc
  unset CC_x86_64_pc_windows_gnu CXX_x86_64_pc_windows_gnu AR_x86_64_pc_windows_gnu
  tgt=${CARGO_TARGET_DIR:-${BP_TMP:-$HOME/.cache/bp-tmp}/target-winmsvc}; export CARGO_TARGET_DIR=$tgt
  (cd "$HERE/native/host" && "$HERE/scripts/cargo-q" xwin build --offline --target x86_64-pc-windows-msvc --profile "$prof" --bin backplane-hub --bin bp-sh)
  (cd "$HERE/native/guard" && "$HERE/scripts/cargo-q" xwin build --offline --target x86_64-pc-windows-msvc --profile release)   # the guard has no fast profile
  # the client with the embedded Chromium (feature cef, as dev-build.sh and release.yml build it; GPUI_FEATURES= for none). The cef crate fetches CEF's
  # Windows distribution once into CEF_PATH and builds its wrapper with clang-cl; the runtime it copies beside the .exe goes into DIR below
  feat=${GPUI_FEATURES-cef}; export CEF_PATH=${CEF_PATH:-$HOME/.cache/backplane/cef-rs}
  (cd "$HERE/native/gpui" && "$HERE/scripts/cargo-q" xwin build --offline --target x86_64-pc-windows-msvc --profile "$prof" ${feat:+--features "$feat"})
  (cd "$HERE/native/voice" && "$HERE/scripts/cargo-q" xwin build --offline --target x86_64-pc-windows-msvc --profile "$prof" --bin backplane-voice)   # dictation's helper, beside the hub
  d=$prof; [ "$d" = dev ] && d=debug
  for f in backplane-hub.exe bp-sh.exe backplane-gpui.exe backplane-voice.exe; do cp "$tgt/x86_64-pc-windows-msvc/$d/$f" "$out/"; done
  [ -z "$feat" ] || "$HERE/scripts/cef-runtime-files.sh" "$tgt/x86_64-pc-windows-msvc/$d" "$out" windows
  cp "$HERE/native/voice/vad/silero_vad.onnx" "$HERE/native/voice/vad/silero_vad.LICENSE" "$out/"   # as release.yml ships it
  cp "$tgt/x86_64-pc-windows-msvc/release/backplane.exe" "$out/"
  git -C "$HERE" rev-parse HEAD > "$out/COMMIT"
  echo "windows-check: $out holds $(cd "$out" && ls | tr '\n' ' ')"
  exit 0
fi
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
