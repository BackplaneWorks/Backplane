#!/bin/bash
# Publish a test build: every dev-reload.sh watching picks it up within seconds.
#   scripts/dev-cut.sh DIR      DIR holds backplane-hub, backplane-gpui, optionally COMMIT (first line names the build) and assets/
#   scripts/dev-cut.sh list     the builds kept; the one marked * is current
#   scripts/dev-cut.sh back     point CURRENT at the build before it that is not marked bad (dev-reload.sh then goes back to it)
# A build is copied whole into $GPUI_DEV_DIR/builds/<id>, then one rename of CURRENT publishes it, so a watcher never sees half a build.
# The last GPUI_DEV_KEEP (default 4) are kept, for going back, and so are CURRENT and whatever a running guard pinned (.pins/: the build it runs and the last good ones), however many lanes cut.
# BAD lists the builds a guard gave up on: `list` marks them !, `back` passes over them. Nothing here starts or stops anything.
set -eu
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}
KEEP=${GPUI_DEV_KEEP:-4}
die() { echo "dev-cut: $*" >&2; exit 1; }
builds() { ls -1 "$DEV/builds" 2>/dev/null | grep -v '^\.' | sort || true; }
publish() { printf '%s\n' "$1" > "$DEV/.CURRENT.$$"; mv -f "$DEV/.CURRENT.$$" "$DEV/CURRENT"; }
current() { head -1 "$DEV/CURRENT" 2>/dev/null || true; }
bad() { cut -f1 "$DEV/BAD" 2>/dev/null || true; }
kept() { current; cat "$DEV"/.pins/* 2>/dev/null || true; }
case "${1:-}" in
  ""|-h|--help) sed -n 2,7p "$0"; exit 2;;
  list) c=$(current); for b in $(builds); do m=" "; if bad | grep -qxF -- "$b"; then m="!"; fi; if [ "$b" = "$c" ]; then echo "*$m$b"; else echo " $m$b"; fi; done; exit 0;;
  back) c=$(current); p=$(builds | grep -B100000 -xF -- "$c" | grep -vxF -f <(bad; echo "$c") | tail -1 || true); [ -n "$p" ] || die "no good build before $c"; publish "$p"; echo "current: $p"; exit 0;;
esac
src=$1
for f in backplane-hub backplane-gpui; do
  [ -x "$src/$f" ] || die "no $f in $src"
  [ "$(head -c4 "$src/$f" | od -An -c | tr -d ' ')" = '177ELF' ] || die "$src/$f is not a program for this machine"
done
# a build without the guard runs its hub bare: no crash restart, no rollback, and fresh-watch.sh reads a dead guard's status (2026-11-09 10:54: a stale builder copy published
# two such builds, and the watcher said the first "did not hold" while it ran unguarded). DEV_CUT_NO_GUARD=1: a test's own dev folder, never the person's.
[ -x "$src/backplane" ] || [ -n "${DEV_CUT_NO_GUARD:-}" ] || die "no guard (backplane) in $src: a build without it runs the hub unguarded (build with scripts/dev-build.sh)"
name=$(head -1 "$src/COMMIT" 2>/dev/null || true)
[ -n "$name" ] || name=$(cat "$src/backplane-hub" "$src/backplane-gpui" | sha256sum | cut -c1-10)
id=$(date +%Y%m%d-%H%M%S)-$(printf '%s' "$name" | tr -c 'A-Za-z0-9._+-' '_')
mkdir -p "$DEV/builds"; chmod 700 "$DEV"
tmp=$DEV/builds/.cut-$$; trap 'rm -rf "${tmp:?}"' EXIT
mkdir "$tmp"; cp "$src/backplane-hub" "$src/backplane-gpui" "$tmp/"
[ ! -x "$src/backplane-browser" ] || cp "$src/backplane-browser" "$tmp/"
[ ! -x "$src/backplane-voice" ] || cp "$src/backplane-voice" "$tmp/"
[ ! -f "$src/silero_vad.onnx" ] || cp "$src/silero_vad.onnx" "$tmp/"
[ ! -x "$src/backplane" ] || cp "$src/backplane" "$tmp/"
for f in libcef.so resources.pak chrome_100_percent.pak chrome_200_percent.pak icudtl.dat v8_context_snapshot.bin locales; do [ ! -e "$src/$f" ] || cp -P "$src/$f" "$tmp/"; done   # links to the shared stripped libcef and its runtime (dev-build.sh)
[ ! -f "$src/COMMIT" ] || cp "$src/COMMIT" "$tmp/"
[ ! -f "$src/HAIKU" ] || cp "$src/HAIKU" "$tmp/"
[ ! -d "$src/assets" ] || cp -r "$src/assets" "$tmp/"
mv "$tmp" "$DEV/builds/$id"; trap - EXIT
publish "$id"
builds | head -n "-$KEEP" | grep -vxF -f <(kept; echo "$id") | while read -r old; do rm -rf "${DEV:?}/builds/${old:?}"; done
echo "cut $id"
