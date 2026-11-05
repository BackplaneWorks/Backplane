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
if ! { pkg-config --exists xkbcommon-x11 && [ -e /usr/lib/x86_64-linux-gnu/libxkbcommon-x11.so ]; }; then
 mkdir -p build/gpui-sysroot/debs
 (cd build/gpui-sysroot/debs && apt-get download libxkbcommon-x11-0 libxkbcommon-x11-dev)
 for deb in build/gpui-sysroot/debs/*.deb; do dpkg-deb -x "$deb" build/gpui-sysroot; done
 rm -f build/gpui-sysroot/usr/lib/x86_64-linux-gnu/*.a
 export PKG_CONFIG_PATH="$PWD/build/gpui-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
 export LIBRARY_PATH="$PWD/build/gpui-sysroot/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
 export CPATH="$PWD/build/gpui-sysroot/usr/include${CPATH:+:$CPATH}"
 export RUSTFLAGS="-L native=$PWD/build/gpui-sysroot/usr/lib/x86_64-linux-gnu ${RUSTFLAGS:-}"
 export LD_LIBRARY_PATH="$PWD/build/gpui-sysroot/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
# The font files are not in git (binaries): fetched once and verified against the committed SHA256SUMS
[ -f assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf ] && [ -f assets/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf ] || sh scripts/gpui/fetch-fonts.sh
# The cats' rigs and the icon set come from Bend (core/cat.bend, core/icons.bend), generated here into
# build/gpui-artifacts/assets/, never hand-copied. The app finds assets/ beside the binary.
BEND=$(command -v bend || echo "$HOME/.bend/bin/bend")
mkdir -p build/gpui-artifacts/assets/cat-rigs
for g in cat_rigs icons; do
 BEND_NO_TELEMETRY=1 "$BEND" native/gpui/gen/$g.bend > build/gpui-artifacts/$g.txt 2> build/gpui-artifacts/$g.err || true
done
python3 - <<'PY'
import json
n = 0
for line in open("build/gpui-artifacts/cat_rigs.txt"):
    parts = line.rstrip("\n").split(" ", 2)
    if len(parts) == 3 and parts[0].isdigit():
        open("build/gpui-artifacts/assets/cat-rigs/%s-%s.json" % (parts[0], parts[1]), "w").write(parts[2]); n += 1
icons = {}
for line in open("build/gpui-artifacts/icons.txt"):
    parts = line.rstrip("\n").split(" ", 2)
    if len(parts) == 3 and parts[0] == "icon":
        icons[parts[1]] = json.loads(parts[2])
json.dump(icons, open("build/gpui-artifacts/assets/icons.json", "w"), separators=(",", ":"))
print("cat rigs:", n, "icons:", len(icons))
PY
cp assets/fonts/jetbrains-mono/OFL.txt build/gpui-artifacts/assets/OFL-JetBrainsMono.txt
cp assets/fonts/jetbrains-mono/SOURCE.txt build/gpui-artifacts/assets/SOURCE-JetBrainsMono.txt
if [ "${1:-}" = check ]; then rc=0; cargo check --release --manifest-path native/gpui/Cargo.toml > build/gpui-artifacts/check.log 2>&1 || rc=$?; grep -E '^(error|warning: unused)' -A12 build/gpui-artifacts/check.log | head -150; echo CHECK-DONE; exit $rc; fi
/usr/bin/time -v -o build/gpui-artifacts/build.time cargo build --release --manifest-path native/gpui/Cargo.toml
cp native/gpui/Cargo.lock build/gpui-artifacts/Cargo.lock
cp build/gpui-target/release/backplane-gpui build/gpui-artifacts/
ldd build/gpui-artifacts/backplane-gpui > build/gpui-artifacts/ldd.txt
stat -c '%s' build/gpui-artifacts/backplane-gpui > build/gpui-artifacts/binary-bytes.txt
/usr/bin/time -v -o build/gpui-artifacts/test.time cargo test --release --manifest-path native/gpui/Cargo.toml > build/gpui-artifacts/tests.txt 2>&1
