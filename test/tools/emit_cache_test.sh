#!/bin/sh
# The emit cache of scripts/build-app.sh, with a stand-in bend (no real
# build): a miss emits and keeps; the same inputs reuse; an edited tracked
# file, a new untracked one, a changed generator or bend miss; a kept emit
# missing its .ok or cut short is never used; a failed emit keeps nothing.
#   sh test/tools/emit_cache_test.sh
set -eu
here=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/bp-emit-test.XXXXXX")
trap 'rm -rf "$tmp"' 0 1 2 15
git clone -q --shared "$here" "$tmp/repo"
cp "$here/scripts/build-app.sh" "$here/scripts/inputs-key.sh" "$tmp/repo/scripts/"
mkdir -p "$tmp/bin" "$tmp/bend2"
echo 'base' >"$tmp/bend2/base.bend"
cat >"$tmp/bin/bend" <<'B'
#!/bin/sh
# stand-in: counts its runs; BEND_FAKE=fail fails half-way, =edit changes a
# source file while it runs
# like bend, it writes C only to a name ending in .c (any other name is a
# native build): refuse anything else so the script can't regress
case $3 in *.c) ;; *) echo "stand-in bend: -o $3 is not a .c file" >&2; exit 3 ;; esac
echo run >>"$BEND_RUNS"
[ "${BEND_FAKE:-}" = fail ] && { echo partial >"$3"; exit 1; }
[ "${BEND_FAKE:-}" = edit ] && echo "# during" >>src/core/json.bend
printf 'emitted %s %s\n' "$1" "$(cat src/core/version.bend | sha256sum | cut -c1-8)" >"$3"
B
chmod +x "$tmp/bin/bend"
export PATH="$tmp/bin:$PATH" BEND_RUNS="$tmp/runs" BACKPLANE_EMIT_CACHE="$tmp/cache" BACKPLANE_EMIT_ONLY=1 BP_BUILD_LOCKED=1 BACKPLANE_CPUS=""
cd "$tmp/repo"
runs() { wc -l <"$BEND_RUNS" 2>/dev/null | tr -d ' ' || echo 0; }
build() { scripts/build-app.sh src/app/main.bend build/x >/dev/null 2>&1; }
fail=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got $2 want $3"; fail=1; fi; }
: >"$BEND_RUNS"
build; check "first build emits" "$(runs)" 1
build; check "same inputs reuse" "$(runs)" 1
check "bend was asked for C (an -o name ending in .c)" "$(test -f build/cc/x/all.c && head -c7 build/cc/x/all.c)" emitted
check "reused C is the emitted C" "$(cat build/cc/x/all.c | cut -c1-7)" emitted
echo "# edit" >>src/core/version.bend; build; check "a tracked edit emits again" "$(runs)" 2
echo 'import Base' >src/core/zz_new.bend; build; check "a new untracked file emits again" "$(runs)" 3
build; check "and is then reused" "$(runs)" 3
echo "# x" >>scripts/cc-split.py; build; check "a changed generator emits again" "$(runs)" 4
printf '\n# changed\n' >>"$tmp/bin/bend"; build; check "a changed bend emits again" "$(runs)" 5
k=$(ls "$BACKPLANE_EMIT_CACHE"/*.ok | head -n1); rm -f "$BACKPLANE_EMIT_CACHE"/*.ok; build; check "an emit without its .ok is not used" "$(runs)" 6
for c in "$BACKPLANE_EMIT_CACHE"/*.c; do printf 'cut' >"$c"; done; build; check "a cut-short emit is not used" "$(runs)" 7
echo "# y" >>src/core/version.bend; n=$(ls "$BACKPLANE_EMIT_CACHE"/*.c | wc -l)
BEND_FAKE=fail build || true; check "a failed emit keeps nothing" "$(ls "$BACKPLANE_EMIT_CACHE"/*.c | wc -l | tr -d ' ')" "$(echo $n | tr -d ' ')"
check "a failed emit leaves no all.c" "$(test -f build/cc/x/all.c && echo there || echo gone)" gone
build; check "after a failed emit the next one emits" "$(runs)" 9
check "at most three kept" "$(ls "$BACKPLANE_EMIT_CACHE"/*.c | wc -l | tr -d ' ')" 3
BACKPLANE_EMIT_CACHE=0 build; check "cache off always emits" "$(runs)" 10
n=$(ls "$BACKPLANE_EMIT_CACHE"/*.c | wc -l | tr -d ' ')
echo "# z" >>src/core/version.bend; BEND_FAKE=edit build; check "an emit during which a source changed is not kept" "$(ls "$BACKPLANE_EMIT_CACHE"/*.c | wc -l | tr -d ' ')" "$n"
git checkout -q src/core/json.bend; build; check "and the next build emits again" "$(runs)" 12
build; check "then reuses" "$(runs)" 12
rm src/core/bom.bend; build; check "a tracked file missing: no key, it emits" "$(runs)" 13
build; check "and still emits (nothing reused without a key)" "$(runs)" 14
git checkout -q src/core/bom.bend
chmod 000 src/core/json.bend; build; check "an unreadable file: no key, it emits" "$(runs)" 15
chmod 644 src/core/json.bend
build; check "readable again: the same state as before, reused" "$(runs)" 15
BEND_LIB="$tmp/otherlib" build; check "another BEND_LIB is another key" "$(runs)" 16
mv "$tmp/bend2" "$tmp/bend2.x"; build; check "no bend library: no key, it emits" "$(runs)" 17
mv "$tmp/bend2.x" "$tmp/bend2"
exit $fail
