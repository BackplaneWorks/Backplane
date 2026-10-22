#!/bin/sh
# Build a Bend program as one executable on Bun's JS runtime: bend emits the
# JS (about 40 s for the server), the version and commit are stamped into
# it, and `bun build --compile` makes a single file (a few seconds), for this
# machine or, cross-compiled, any target Bun has. The hub is built this way
# everywhere: scripts/build.sh, scripts/dev.sh, CI's js job and releases
# (scripts/service-release.py js).
#
#   scripts/build-js.sh src/server/main.bend dist/backplane-serve
#   BACKPLANE_JS_TARGET=windows-x64 scripts/build-js.sh src/server/main.bend dist/backplane-serve
#                     (writes dist/backplane-serve.exe)
#
#   BACKPLANE_VERSION_STAMP / BACKPLANE_COMMIT_STAMP  the version and commit
#                     stamped into a copy of the emit (scripts/stamp-version.py)
#   BACKPLANE_JS_TARGET  a Bun target without "bun-" (linux-x64, linux-arm64,
#                     darwin-x64, darwin-arm64, windows-x64, ...); default
#                     this machine
#   BACKPLANE_TELEMETRY_BAKE=1 / BACKPLANE_GOOGLE_BAKE  the
#                     release's credentials go into a private prelude that
#                     is compiled in (scripts/*-header.py writing .js) and
#                     removed after; the emitted and kept JS never hold them
#   BACKPLANE_EMIT_CACHE  the emitted JS kept by a hash of everything bend
#                     reads (default ~/.cache/backplane-bend/emit-js; 0 =
#                     none); three are kept
# Prints one "stages:" line: emit (fresh or reused), stamp, compile seconds
# and the emit's peak RSS.
set -eu
cd "$(dirname "$0")/.."
# the key the emit is kept under: only what bend reads for $src (its
# imports, effects and headers), so a change elsewhere (the web client, a
# script) reuses the last emit
emit_key() {
  BP_KEY_ROOTS=$1 scripts/inputs-key.sh "emit-js1:$1" scripts/inputs-key.sh scripts/bend-closure.py
}
if [ "${1:-}" = --key ]; then
  emit_key "${2:?build-js.sh --key SRC}"
  exit
fi
src=$1
out=$2
export BEND_NO_TELEMETRY=1
# bend is a Bun program: JavaScriptCore sizes its heap to the machine's RAM.
# BUN_JSC_forceRAMSize tells it a 4 GiB machine (a sizing hint, not a cap),
# which keeps the emit near 8 GB at about the same speed
export BUN_JSC_forceRAMSize=${BUN_JSC_forceRAMSize:-4294967296}
bun=$(command -v bun || echo "$HOME/.bun/bin/bun")
[ -x "$bun" ] || { echo "build-js.sh: bun is needed" >&2; exit 1; }
now_ms() { echo $(( $(date +%s%N) / 1000000 )); }
work="build/js/$(basename "$out")"
mkdir -p "$work"
t0=$(now_ms)
EMIT_CACHE=${BACKPLANE_EMIT_CACHE-${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/emit-js}
[ "$EMIT_CACHE" = 0 ] && EMIT_CACHE=""
key=""
if [ -n "$EMIT_CACHE" ]; then
  key=$(emit_key "$src" 2>/dev/null || true)
fi
how=fresh
rm -f "$work/emit.rss"
if [ -n "$key" ] && [ -f "$EMIT_CACHE/$key.js" ] && [ -f "$EMIT_CACHE/$key.js.ok" ] \
    && [ "$(cat "$EMIT_CACHE/$key.js.ok")" = "$(wc -c <"$EMIT_CACHE/$key.js" | tr -d ' ')" ]; then
  cp "$EMIT_CACHE/$key.js" "$work/all.tmp.js" && mv "$work/all.tmp.js" "$work/all.js"
  touch "$EMIT_CACHE/$key.js" "$EMIT_CACHE/$key.js.ok"
  how=reused
else
  if [ -x /usr/bin/time ]; then
    /usr/bin/time -f %M -o "$work/emit.rss" nice -n 19 bend "$src" -o "$work/all.tmp.js" >/dev/null
  else
    nice -n 19 bend "$src" -o "$work/all.tmp.js" >/dev/null
  fi
  mv "$work/all.tmp.js" "$work/all.js"
  # kept only if nothing it read changed while it ran
  if [ -n "$key" ] && [ "$(emit_key "$src" 2>/dev/null || true)" = "$key" ]; then
    mkdir -p "$EMIT_CACHE"
    cp "$work/all.js" "$EMIT_CACHE/$key.js.$$" && mv "$EMIT_CACHE/$key.js.$$" "$EMIT_CACHE/$key.js" \
      && wc -c <"$EMIT_CACHE/$key.js" | tr -d ' ' >"$EMIT_CACHE/$key.js.ok.$$" && mv "$EMIT_CACHE/$key.js.ok.$$" "$EMIT_CACHE/$key.js.ok"
    ls -t "$EMIT_CACHE"/*.js 2>/dev/null | tail -n +4 | while read -r old; do rm -f "$old" "$old.ok"; done
  fi
fi
t1=$(now_ms)
# the stamps go into a copy, so the kept emit is the same for every version
cp "$work/all.js" "$work/stamped.js"
if [ -n "${BACKPLANE_VERSION_STAMP:-}" ]; then
  python3 scripts/stamp-version.py "$work/stamped.js" "$BACKPLANE_VERSION_STAMP"
fi
if [ -n "${BACKPLANE_COMMIT_STAMP:-}" ]; then
  python3 scripts/stamp-version.py "$work/stamped.js" "$BACKPLANE_COMMIT_STAMP" commit
fi
# Base's String.cmp and Map.bit.go swapped for native ones, as for the web
# (scripts/js-native.py): Base's String.cmp is not a tail call, and on long
# strings it overflowed the hub's stack (fast-hub J2: a memory fault right
# after project.add on an empty home). js-native.py finds them by the
# minified shape, so the emit is minified (names kept) first.
"$bun" build --minify-whitespace --minify-syntax --keep-names --target=bun \
  "$work/stamped.js" --outfile "$work/native.js" >/dev/null
python3 scripts/js-native.py --server "$work/native.js" >/dev/null
mv -f "$work/native.js" "$work/stamped.js"
# the baked prelude, then the program: one file for bun to compile
# removed on every way out: done, a failure (set -e) or a signal
unbake() { rm -f "$work/baked.js" "$work/bin.js" "$work/baked-t.js" "$work/baked-g.js"; }
trap unbake EXIT
trap 'unbake; exit 129' HUP
trap 'unbake; exit 130' INT
trap 'unbake; exit 143' TERM
rm -f "$work/baked.js" "$work/bin.js"
python3 scripts/telemetry-header.py "$work/baked-t.js"
# the baked Google client, read from .env (here or in the main checkout)
# without running it
envval() {
  for f in .env "$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)/../.env"; do
    [ -f "$f" ] || continue
    v=$(sed -n "s/^[[:space:]]*\(export[[:space:]]\{1,\}\)\{0,1\}$1=//p" "$f" | tail -n1 | tr -d "\"'\r")
    [ -n "$v" ] && { printf %s "$v"; return; }
  done
}
if [ "${BACKPLANE_GOOGLE_BAKE:-1}" != 0 ]; then
  BACKPLANE_GOOGLE_CLIENT_ID=${BACKPLANE_GOOGLE_CLIENT_ID:-$(envval BACKPLANE_GOOGLE_CLIENT_ID)} \
  BACKPLANE_GOOGLE_CLIENT_SECRET=${BACKPLANE_GOOGLE_CLIENT_SECRET:-$(envval BACKPLANE_GOOGLE_CLIENT_SECRET)} \
    python3 scripts/google-header.py "$work/baked-g.js"
else
  BACKPLANE_GOOGLE_BAKE=0 python3 scripts/google-header.py "$work/baked-g.js"
fi
# the stack prelude first (scripts/js-stack-prelude.js: a 4 GiB stack and
# Bun's matching JS stack bound, set by executing itself once), then the
# baked values, then the program
(umask 077; cat "$work/baked-t.js" "$work/baked-g.js" >"$work/baked.js"; cat scripts/js-stack-prelude.js "$work/baked.js" "$work/stamped.js" >"$work/bin.js")
rm -f "$work/baked-t.js" "$work/baked-g.js"
grep -q '"BACKPLANE_GOOGLE_CLIENT_ID": "[^"]' "$work/baked.js" && echo "baked Google client"
grep -q '"BACKPLANE_TELEMETRY_KEY": "[^"]' "$work/baked.js" && echo "baked telemetry"
t2=$(now_ms)
target=${BACKPLANE_JS_TARGET:-}
case $target in
  "") tflag="" ;;
  *[!a-z0-9-]*) echo "build-js.sh: BACKPLANE_JS_TARGET=$target" >&2; exit 1 ;;
  *) tflag="--target=bun-$target" ;;
esac
case $target in windows-*) out="${out%.exe}.exe" ;; esac
# a release pins the target's Bun runtime (scripts/service-release.py js);
# otherwise bun fetches the one of its own version for the target
rt=""
[ -n "${BACKPLANE_JS_BUN_RUNTIME:-}" ] && rt="--compile-executable-path=$BACKPLANE_JS_BUN_RUNTIME"
mkdir -p "$(dirname "$out")"
# whitespace and syntax minified, names kept: a crash's stack names Bend's
# defs and the effects. The hub never reads a .env, bunfig.toml or tsconfig
# of the folder it starts in: those are a project's, not its configuration.
"$bun" build --compile --minify-whitespace --minify-syntax --keep-names \
  --no-compile-autoload-dotenv --no-compile-autoload-bunfig \
  $tflag $rt "$work/bin.js" --outfile "$out.tmp" >/dev/null
case $target in windows-*) mv -f "$out.tmp.exe" "$out" 2>/dev/null || mv -f "$out.tmp" "$out" ;; *) mv -f "$out.tmp" "$out" ;; esac
unbake
t3=$(now_ms)
sec() { awk -v a="$1" -v b="$2" 'BEGIN { printf "%.1f", (b - a) / 1000 }'; }
rss=$(grep -E '^[0-9]+$' "$work/emit.rss" 2>/dev/null | tail -n1 || true)
peak=$( [ -n "$rss" ] && awk -v k="$rss" 'BEGIN { printf "%.2fG", k / 1048576 }' || printf -- - )
line="stages: ${target:-native} emit ($how) $(sec "$t0" "$t1")s stamp $(sec "$t1" "$t2")s compile $(sec "$t2" "$t3")s total $(sec "$t0" "$t3")s | peak RSS: emit $peak"
echo "$line" >"$work/stages.txt"
echo "$line"
echo "$out"
