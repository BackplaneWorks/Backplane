#!/bin/sh
# One hash of what a build or a gate reads, for the caches that reuse their
# results (scripts/build-js.sh's emit, the verified records of check.sh and
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
# With BP_KEY_ROOTS="A.bend B.bend", the checkout's files are only those the
# roots read (their imports, transitively; scripts/bend-closure.py), so a
# change elsewhere leaves the key as it was.
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
trap 'rm -f "$tmp" "$tmp.l"' 0 1 2 15
# The official builds of one bend release are one compiler for every
# platform (the same program, bundled per platform; their bend2/ folders
# are byte for byte the same), so they key alike and an emit made on one
# machine serves another. Any other binary keys by its own bytes.
bin=$(sha256sum "$real" | cut -c1-64) || exit 1
case $bin in
  b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643|\
  301a69a8e20937b37a431ecff5a80e85fdc5a2224ead972088e4ee9f3f89a300|\
  4d7280f366661d9b63b418eb70ca54ea1a867a23b196cc4b5f1f75e049cb80d1|\
  db96d899b65a1a1415291bfb9fb33d1ab43b0d076071d3489cc4a678670b2e00)
    bin="release 2.0.36" ;; # linux-x64, linux-arm64, darwin-arm64, darwin-x64
esac
{ printf 'inputs-v3\ntag %s\nBEND_LIB=%s\nBENDTT=%s\nCC=%s\nbend %s\n' "$tag" "${BEND_LIB-}" "${BENDTT-}" "${CC-}" "$bin"
} >"$tmp" || exit 1
# bend's own files by their place under bend2/ (and the library folder),
# not by where this machine keeps them
( cd "$dir" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 -r sha256sum ) >>"$tmp" || exit 1
if [ -d "$lib" ]; then
  printf 'lib\n' >>"$tmp"
  ( cd "$lib" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 -r sha256sum ) >>"$tmp" || exit 1
fi
for f in "$@"; do
  printf 'named %s\n' "$f" >>"$tmp" || exit 1
  sha256sum "$f" >>"$tmp" || exit 1
done
if [ -n "${BP_KEY_ROOTS:-}" ]; then
  # only what those programs read (scripts/bend-closure.py), not the whole
  # checkout: a key for a gate over these roots alone
  printf 'roots %s\n' "$BP_KEY_ROOTS" >>"$tmp" || exit 1
  # shellcheck disable=SC2086
  python3 scripts/bend-closure.py $BP_KEY_ROOTS | tr '\n' '\0' >"$tmp.l" || exit 1
  [ -s "$tmp.l" ] || exit 1
else
  git ls-files -z --cached --others --exclude-standard -- '*.bend' '*.c' '*.h' '*.js' \
    ':(exclude).bend-check.*' ':(exclude).bend-test.*' >"$tmp.l" || exit 1
fi
LC_ALL=C sort -z "$tmp.l" -o "$tmp.l" || exit 1
xargs -0 -r sha256sum <"$tmp.l" >>"$tmp" || exit 1
sha256sum <"$tmp" | cut -c1-40
