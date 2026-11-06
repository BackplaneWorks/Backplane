#!/bin/bash
# Publish a test build: every dev-reload.sh watching picks it up within seconds.
#   scripts/gpui/dev-cut.sh DIR      DIR holds backplane-hub, backplane-gpui, optionally COMMIT (first line names the build) and assets/
#   scripts/gpui/dev-cut.sh list     the builds kept; the one marked * is current
#   scripts/gpui/dev-cut.sh back     point CURRENT at the build before it (dev-reload.sh then goes back to it)
# A build is copied whole into $GPUI_DEV_DIR/builds/<id>, then one rename of CURRENT publishes it, so a watcher never sees half a build.
# The last GPUI_DEV_KEEP (default 4) are kept, for going back. Nothing here starts, stops or reads anything else.
set -eu
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}
KEEP=${GPUI_DEV_KEEP:-4}
die() { echo "dev-cut: $*" >&2; exit 1; }
builds() { ls -1 "$DEV/builds" 2>/dev/null | grep -v '^\.' | sort || true; }
publish() { printf '%s\n' "$1" > "$DEV/.CURRENT.$$"; mv -f "$DEV/.CURRENT.$$" "$DEV/CURRENT"; }
current() { head -1 "$DEV/CURRENT" 2>/dev/null || true; }
case "${1:-}" in
  ""|-h|--help) sed -n 2,7p "$0"; exit 2;;
  list) c=$(current); for b in $(builds); do if [ "$b" = "$c" ]; then echo "* $b"; else echo "  $b"; fi; done; exit 0;;
  back) c=$(current); p=$(builds | grep -B1 -xF -- "$c" | head -1 || true); [ -n "$p" ] && [ "$p" != "$c" ] || die "no build before $c"; publish "$p"; echo "current: $p"; exit 0;;
esac
src=$1
for f in backplane-hub backplane-gpui; do
  [ -x "$src/$f" ] || die "no $f in $src"
  [ "$(head -c4 "$src/$f" | od -An -c | tr -d ' ')" = '177ELF' ] || die "$src/$f is not a program for this machine"
done
name=$(head -1 "$src/COMMIT" 2>/dev/null || true)
[ -n "$name" ] || name=$(cat "$src/backplane-hub" "$src/backplane-gpui" | sha256sum | cut -c1-10)
id=$(date +%Y%m%d-%H%M%S)-$(printf '%s' "$name" | tr -c 'A-Za-z0-9._+-' '_')
mkdir -p "$DEV/builds"; chmod 700 "$DEV"
tmp=$DEV/builds/.cut-$$; trap 'rm -rf "${tmp:?}"' EXIT
mkdir "$tmp"; cp "$src/backplane-hub" "$src/backplane-gpui" "$tmp/"
[ ! -f "$src/COMMIT" ] || cp "$src/COMMIT" "$tmp/"
[ ! -d "$src/assets" ] || cp -r "$src/assets" "$tmp/"
mv "$tmp" "$DEV/builds/$id"; trap - EXIT
publish "$id"
builds | head -n "-$KEEP" | while read -r old; do rm -rf "${DEV:?}/builds/${old:?}"; done
echo "cut $id"
