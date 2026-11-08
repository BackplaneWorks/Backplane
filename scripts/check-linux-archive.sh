#!/bin/sh
# check-linux-archive.sh ARCHIVE.tar.gz : unpack a Linux release archive and fail if the embedded browser could not start from it.
# CEF reads icudtl.dat, the paks, the V8 snapshot and locales/ from the directory libcef.so was loaded from, and the client loads libcef.so through RUNPATH $ORIGIN:
# all of them must sit beside backplane-gpui (a missing one is a failed CHECK inside cef_initialize that kills the window). Used by release.yml on every Linux build.
set -eu
A=$(realpath "$1"); T=$(mktemp -d "${TMPDIR:-/tmp}/bp-archcheck.XXXXXX"); trap 'rm -rf "$T"' EXIT
tar -C "$T" -xzf "$A"
bad=0; miss() { echo "::error::$(basename "$A"): $1"; bad=1; }
for f in backplane-gpui libcef.so icudtl.dat resources.pak chrome_100_percent.pak chrome_200_percent.pak v8_context_snapshot.bin locales/en-US.pak; do
  [ -s "$T/$f" ] || miss "$f missing or empty beside backplane-gpui"
done
[ "$(ls "$T/locales" 2>/dev/null | grep -c '\.pak$')" -ge 20 ] || miss "locales/ has fewer than 20 .pak files"
[ ! -L "$T/libcef.so" ] || miss "libcef.so is a link (CEF would look for its files beside the link's target)"
if command -v readelf > /dev/null; then
  readelf -d "$T/backplane-gpui" | grep -q 'NEEDED.*\[libcef\.so\]' || miss "backplane-gpui does not link libcef.so (built without the cef feature)"
  readelf -d "$T/backplane-gpui" | grep -Eq '(RUNPATH|RPATH).*\$ORIGIN' || miss "backplane-gpui has no \$ORIGIN runpath (libcef.so beside it would not be found)"
fi
# the loader resolves libcef.so to the archive's own copy, with no LD_LIBRARY_PATH (a runner of the archive's architecture only)
if [ "$(uname -m)" = "$(readelf -h "$T/backplane-gpui" 2>/dev/null | sed -n 's/.*Machine:.*X86-64.*/x86_64/p; s/.*Machine:.*AArch64.*/aarch64/p')" ]; then
  L=$(env -u LD_LIBRARY_PATH ldd "$T/backplane-gpui" 2>/dev/null | awk '/libcef\.so/{print $3}')
  [ "$L" = "$T/libcef.so" ] || miss "libcef.so resolves to '${L:-nothing}', not the archive's own"
fi
[ $bad = 0 ] && echo "ok $(basename "$A"): libcef.so, icudtl.dat, resources.pak, chrome_{100,200}_percent.pak, v8_context_snapshot.bin and $(ls "$T/locales" | wc -l) locales beside backplane-gpui"
exit $bad
