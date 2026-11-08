#!/usr/bin/env bash
# Put licenses/ into a package folder: the notices of every third-party component that folder ships (deploy/licenses/README.md lists them).
# release.yml runs it in both package steps, after the programs are in place and tools/*/node_modules are installed, and fails the job when it does.
# usage: scripts/licenses.sh PKG_DIR TARGET [RUST_DIR]
#   TARGET   linux-x64 | linux-arm64 | darwin-arm64 | windows-x64
#   RUST_DIR the output of scripts/rust-licenses.sh for TARGET (default build/licenses/rust); LICENSES_NO_RUST=1 skips it (a dev cut without cargo-about)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
pkg=${1:?package dir}; t=${2:?target}; rust=${3:-$root/build/licenses/rust}
src=$root/deploy/licenses; L=$pkg/licenses
die() { echo "::error::licenses: $*" >&2; exit 1; }
rm -rf "$L"; mkdir -p "$L"
put() { # put DEST_SUBDIR FILE... : each file must exist
  local d=$L/$1; shift; mkdir -p "$d"
  for f in "$@"; do [ -s "$f" ] || die "${f#$root/} missing"; cp "$f" "$d/"; done
}
cp "$src/README.md" "$L/README.md"; cp "$root/LICENSE" "$L/LICENSE"

# Bun: the notice names the pinned version, and the Bun that compiled the two programs must be that one
pin=$(tr -d ' \r\n' < "$root/deploy/bun-version")
grep -q "Bun $pin " "$src/bun/NOTICE.md" || die "deploy/licenses/bun/NOTICE.md does not name Bun $pin (deploy/bun-version): update the notices with the pin"
grep -q "bun-v$pin" "$src/bun/NOTICE.md" || die "deploy/licenses/bun/NOTICE.md does not name bun-v$pin"
have=$(${BUN:-bun} --version 2>/dev/null | tr -d ' \r\n' || true)
[ "$have" = "$pin" ] || die "the Bun on PATH is ${have:-missing}, the notices are for $pin (deploy/bun-version)"
put bun "$src/bun/NOTICE.md" "$src/bun/LICENSE.md"
pw=$root/tools/browser/node_modules/playwright-core
put playwright-core "$pw/LICENSE" "$pw/NOTICE" "$pw/ThirdPartyNotices.txt"
oc=$root/tools/step2glb/node_modules/occt-import-js
put step2glb "$oc/dist/license.occt.txt" "$oc/dist/license.occt-import-js.txt"

# CEF (every desktop target: the client embeds it): CEF's licence, and Chromium's credits from the distribution the build used (the cef crate's OUT_DIR,
# else the copy cef-dll-sys put beside the binary)
case "$t" in linux-*|darwin-*|windows-*)
  # GPUI_RELEASE: where the client was built (default native/gpui/target/release)
  rel=${GPUI_RELEASE:-$root/native/gpui/target/release}
  credits=$(find "$rel/build" -path '*cef-dll-sys*' -name CREDITS.html 2>/dev/null | head -1 || true)
  [ -n "$credits" ] || credits=$rel/CREDITS.html
  put cef "$src/cef/LICENSE.txt" "$credits" ;;
esac

put silero-vad "$src/silero-vad/LICENSE"
put voice-engine "$src/voice-engine/NOTICE.md" "$src/voice-engine/sherpa-onnx-LICENSE" "$src/voice-engine/onnxruntime-LICENSE" "$src/voice-engine/CC-BY-4.0.txt"
put fonts "$root/assets/fonts/jetbrains-mono/OFL.txt" "$root/assets/fonts/SPLEEN-LICENSE"
put icons "$src/icons/lucide-LICENSE"; cp "$root/assets/providers/NOTICE.md" "$L/icons/simple-icons.md"
put gpui "$root/native/gpui/vendor/gpui/LICENSE-APACHE" "$root/native/gpui/vendor/gpui/PATCH.md"

if [ "${LICENSES_NO_RUST:-}" = 1 ]; then echo "licenses: no Rust notices (LICENSES_NO_RUST=1)" >&2
else put rust "$rust/hub.txt" "$rust/client.txt" "$rust/guard.txt" "$rust/voice.txt"; fi
echo "licenses: $(find "$L" -type f | wc -l | tr -d ' ') files in ${L#$root/}"
