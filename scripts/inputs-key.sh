#!/bin/sh
# One hash of what a build or a gate reads, for the caches that reuse their
# results (scripts/build-app.sh's emit, the verified records of check.sh and
# test.sh). It covers:
#   - bend itself: the binary bend resolves to, and the files it reads beside
#     it (<binary's folder>/../bend2: base.bend, its effects, the proof
#     kernel's source), its named-library folder (BEND_LIB, default
#     ~/.bend/lib) when there is one, and the settings bend reads that change
#     what it makes (BEND_LIB, BENDTT: the proof kernel, CC);
#   - every tracked or untracked (not ignored) .bend/.c/.h/.js file of the
#     checkout, except the gates' own work folders (.bend-check.*,
#     .bend-test.*), which exist only while they run; ignored files are not
#     inputs (no program imports from build/ or dist/);
#   - the files named, and the tag that names what the key is for.
# It fails closed: any step that cannot read what it must (no git checkout,
# no bend, a tracked file missing, a file it cannot hash) prints nothing and
# exits 1, so nothing is reused.
#   scripts/inputs-key.sh TAG [FILE...]
set -u
cd "$(dirname "$0")/.." || exit 1
tag=${1:?inputs-key.sh: a tag}
shift
b=$(command -v bend) || exit 1
real=$(readlink -f "$b") || exit 1
dir=$(dirname "$real")/../bend2
[ -f "$dir/base.bend" ] || exit 1
lib=${BEND_LIB:-$HOME/.bend/lib}
git ls-files --error-unmatch scripts/inputs-key.sh >/dev/null 2>&1 || exit 1
tmp=$(mktemp "${TMPDIR:-/tmp}/bp-key.XXXXXX") || exit 1
trap 'rm -f "$tmp" "$tmp.l" "$tmp.b"' 0 1 2 15
{ printf 'inputs-v2\ntag %s\nBEND_LIB=%s\nBENDTT=%s\nCC=%s\n' "$tag" "${BEND_LIB-}" "${BENDTT-}" "${CC-}"
  sha256sum "$real"
} >"$tmp" || exit 1
find "$dir" -type f -print0 | sort -z >"$tmp.b" || exit 1
[ -d "$lib" ] && { find "$lib" -type f -print0 | sort -z >>"$tmp.b" || exit 1; }
xargs -0 -r sha256sum <"$tmp.b" >>"$tmp" || exit 1
for f in "$@"; do
  printf 'named %s\n' "$f" >>"$tmp" || exit 1
  sha256sum "$f" >>"$tmp" || exit 1
done
git ls-files -z --cached --others --exclude-standard -- '*.bend' '*.c' '*.h' '*.js' \
  ':(exclude).bend-check.*' ':(exclude).bend-test.*' >"$tmp.l" || exit 1
sort -z "$tmp.l" -o "$tmp.l" || exit 1
xargs -0 -r sha256sum <"$tmp.l" >>"$tmp" || exit 1
sha256sum <"$tmp" | cut -c1-40
