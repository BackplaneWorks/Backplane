# Sourced. win_mingw_env: the C toolchain a Windows-target cargo check needs (crates' C code, the client's manifest), for x86_64-pc-windows-gnu:
# llvm-mingw (github.com/mstorsjo/llvm-mingw, ucrt-ubuntu-*-x86_64), WIN_LLVM_MINGW, else the newest ~/.local/opt/llvm-mingw-*; on a Boat box
# (BP_ON_BOAT, set by scripts/lib-boat.sh's jobs) or with WIN_MINGW_FETCH=1 the release WIN_LLVM_MINGW_REL (20261104, the desktop's) is
# fetched into ~/.local/opt once; and rustup's x86_64-pc-windows-gnu target, added when missing. Exports CC/CXX/AR/RC for the target.
# Returns 2, saying what is missing, when it cannot.
win_mingw_env() {
  local tc rel n
  tc=${WIN_LLVM_MINGW:-$(ls -d "$HOME"/.local/opt/llvm-mingw-* 2>/dev/null | sort | tail -1)}
  if [ ! -x "$tc/bin/x86_64-w64-mingw32-clang" ] && [ -n "${BP_ON_BOAT:-}${WIN_MINGW_FETCH:-}" ]; then
    rel=${WIN_LLVM_MINGW_REL:-20261104}; n=llvm-mingw-$rel-ucrt-ubuntu-22.04-x86_64; mkdir -p "$HOME/.local/opt"
    echo "windows-check: fetching llvm-mingw $rel into ~/.local/opt (once)" >&2
    curl -fsSL "https://github.com/mstorsjo/llvm-mingw/releases/download/$rel/$n.tar.xz" | tar -xJ -C "$HOME/.local/opt" && tc=$HOME/.local/opt/$n
  fi
  [ -x "$tc/bin/x86_64-w64-mingw32-clang" ] || { echo "windows-check: no llvm-mingw (set WIN_LLVM_MINGW, unpack one into ~/.local/opt, or WIN_MINGW_FETCH=1)" >&2; return 2; }
  rustup target list --installed | grep -qx x86_64-pc-windows-gnu || rustup target add x86_64-pc-windows-gnu >&2 || { echo "windows-check: rustup target add x86_64-pc-windows-gnu failed" >&2; return 2; }
  export CC_x86_64_pc_windows_gnu=$tc/bin/x86_64-w64-mingw32-clang CXX_x86_64_pc_windows_gnu=$tc/bin/x86_64-w64-mingw32-clang++ AR_x86_64_pc_windows_gnu=$tc/bin/llvm-ar
  # the client's manifest (vendored gpui embeds it for a Windows target built here): llvm-mingw's windres
  export RC_x86_64_pc_windows_gnu=$tc/bin/x86_64-w64-mingw32-windres RC_x86_64_pc_windows_gnullvm=$tc/bin/x86_64-w64-mingw32-windres
  WIN_TC=$tc
}
