#!/bin/bash
# build-archive.sh SRC BASE_ARCHIVE OUT_DIR TAG: on a BUILD box (from the bp-build-base snapshot, never the clean gate box), build the commit in
# SRC (a `git archive` of it: no .git) and pack it the way release.yml packs a Linux release, so the gate can install it as a person would.
# The Rust programs (hub, window, guard, voice), assets and the CEF runtime the window was built against come from this build; the parts
# built by other toolchains (backplane-browser, backplane-step2glb, kistack, licenses, silero_vad.onnx) are taken from BASE_ARCHIVE (the newest
# release's archive), which says so in OUT_DIR/OVERLAY. Writes OUT_DIR/backplane-TAG-linux-x64-native.tar.gz and OUT_DIR/install.sh
# (deploy/install.sh.in filled in for it, as release.yml does). PROFILE: fast (default) or release.
set -euo pipefail
SRC=$(realpath "$1"); BASE=$(realpath "$2"); OUT=$(realpath -m "$3"); TAG=$4; PROFILE=${PROFILE:-fast}
[ "$(id -un)" = user ] || { echo "build-archive.sh: runs on a Boat build box only; refusing" >&2; exit 3; }
case $TAG in v[0-9]*) ;; *) echo "TAG must look like v0.16.0-dev.abc1234 (install.sh refuses others)" >&2; exit 2;; esac
mkdir -p "$OUT"; T=$HOME/target-bare; mkdir -p "$T"
J=$(( $(nproc) )); export CARGO_INCREMENTAL=0
command -v sccache >/dev/null && export RUSTC_WRAPPER=sccache
b() { (cd "$SRC/native/$1" && CARGO_TARGET_DIR="$T/$1" cargo build --profile "$PROFILE" -j "$J" --locked "${@:2}"); }
s=$(date +%s)
b host --bin backplane-hub; b guard; b voice --bin backplane-voice; b gpui --features cef   # as release.yml builds them
echo "built in $(( $(date +%s) - s )) s (profile $PROFILE)"
d=$OUT/pkg; rm -rf "$d"; mkdir -p "$d"; tar -C "$d" -xzf "$BASE"
for f in backplane-hub:host backplane-gpui:gpui backplane:guard backplane-voice:voice; do
  n=${f%%:*}; c=${f#*:}; cp "$T/$c/$PROFILE/$n" "$d/$n"
done
R=$(dirname "$(find "$T/gpui/$PROFILE/build" -path '*cef-dll-sys*' -name libcef.so | head -1)")
[ -f "$R/libcef.so" ] || { echo "no CEF runtime in the build" >&2; exit 1; }
rm -rf "$d/locales"; for f in libcef.so resources.pak chrome_100_percent.pak chrome_200_percent.pak icudtl.dat v8_context_snapshot.bin; do cp "$R/$f" "$d/"; done
for f in libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json; do [ ! -f "$R/$f" ] || cp "$R/$f" "$d/"; done
cp -r "$R/locales" "$d/locales"; strip --strip-unneeded "$d/libcef.so"
# the client's generated assets: this commit's source assets over the release's (icons.json and the cat rigs are generated, the release has them)
cp -r "$SRC/assets/." "$d/assets/"
cat "$SRC/COMMIT" > "$d/COMMIT" 2>/dev/null || true
printf 'Rust programs, assets and CEF from %s (profile %s); backplane-browser, backplane-step2glb, kistack, licenses from %s\n' "$(cat "$d/COMMIT")" "$PROFILE" "$(basename "$BASE")" > "$d/OVERLAY"
name=backplane-$TAG-linux-x64-native.tar.gz
tar -C "$d" -czf "$OUT/$name" .; rm -rf "$d"
sum=$(sha256sum "$OUT/$name" | cut -d' ' -f1)
sed -e "s|@REPO@|BackplaneWorks/Backplane|" -e "s|@VERSION@|$TAG|" -e "s|@SHA_LINUX_X64@|$sum|" -e "s|@SHA_LINUX_ARM64@||" -e "s|@SHA_DARWIN_ARM64@||" \
  "$SRC/deploy/install.sh.in" > "$OUT/install.sh"
sh -n "$OUT/install.sh"
echo "archive $OUT/$name ($(du -m "$OUT/$name" | cut -f1) MB) sha256 $sum"
