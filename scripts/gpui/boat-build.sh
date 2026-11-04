#!/bin/sh
# This script is uploaded by boat-job.sh. Never run a toolchain locally.
set -eu
[ "${BACKPLANE_HEAVY_BOAT_SANDBOX:-}" = 1 ] || { echo 'GPUI builds run only on Boat' >&2; exit 75; }
CAP=8589934592
CHECK='g=$(cut -d: -f3 /proc/self/cgroup); b=/sys/fs/cgroup$g; mx=$(cat "$b/memory.max"); sw=$(cat "$b/memory.swap.max"); case "$mx" in ""|*[!0-9]*) exit 1;; esac; [ "$mx" -gt 0 ] && [ "$mx" -le 8589934592 ] && [ "$sw" = 0 ]'
if ! sh -c "$CHECK"; then
 [ "${GPUI_CAP_TRIED:-}" != 1 ] || { echo "kernel memory cap unavailable; refusing" >&2; exit 75; }
 export GPUI_CAP_TRIED=1
 exec systemd-run --user --scope -q -p MemoryAccounting=yes -p MemoryMax=$CAP -p MemorySwapMax=0 -- sh "$0" "$@"
fi
mkdir -p build/gpui-artifacts
sh -c 'g=$(cut -d: -f3 /proc/self/cgroup); b=/sys/fs/cgroup$g; cat "$b/memory.max" "$b/memory.swap.max"' > build/gpui-artifacts/cap.txt
# Only existing unprivileged tools. Missing system packages are downloaded
# and extracted locally; no sudo or administrator command is attempted.
export CARGO_HOME="$HOME/.cargo" CARGO_TARGET_DIR="$PWD/build/gpui-target" CARGO_BUILD_JOBS=1
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null || { echo 'Boat image lacks Rust' >&2; exit 75; }
rustc --version > build/gpui-artifacts/toolchain.txt
cargo --version >> build/gpui-artifacts/toolchain.txt
if ! pkg-config --exists xkbcommon; then
 mkdir -p build/gpui-sysroot/debs
 (cd build/gpui-sysroot/debs && apt-get download libxkbcommon-dev libxkbcommon-x11-dev libxcb-xkb-dev libxcb1-dev libwayland-dev)
 for deb in build/gpui-sysroot/debs/*.deb; do dpkg-deb -x "$deb" build/gpui-sysroot; done
 export PKG_CONFIG_PATH="$PWD/build/gpui-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
 export LIBRARY_PATH="$PWD/build/gpui-sysroot/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
 export CPATH="$PWD/build/gpui-sysroot/usr/include${CPATH:+:$CPATH}"
fi
/usr/bin/time -v -o build/gpui-artifacts/build.time cargo build --release --manifest-path native/gpui/Cargo.toml
cp native/gpui/Cargo.lock build/gpui-artifacts/Cargo.lock
cp build/gpui-target/release/backplane-gpui build/gpui-artifacts/
ldd build/gpui-artifacts/backplane-gpui > build/gpui-artifacts/ldd.txt
stat -c '%s' build/gpui-artifacts/backplane-gpui > build/gpui-artifacts/binary-bytes.txt
/usr/bin/time -v -o build/gpui-artifacts/test.time cargo test --release --manifest-path native/gpui/Cargo.toml > build/gpui-artifacts/tests.txt 2>&1
