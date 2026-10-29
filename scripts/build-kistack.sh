#!/bin/sh
# KiStack's skills, bundled with the app as the offline fallback: the
# pinned commit (deploy/kistack.lock) fetched at build time, its skills/
# tree checked against the pinned sha256, and only skills/** and its LICENSE
# kept, with a plugin.json of our own. Nothing else is taken from the
# repository (hooks, .mcp.json, its own manifest): Claude loads the result
# with --plugin-dir. The hub syncs the newest commit into <home>/kistack at
# each start the same way (src/server/kicad.bend's KiStack.sync; the
# manifest text is KiStack.manifest), and that copy wins once complete.
#   scripts/build-kistack.sh DEST      (e.g. dist/kistack)
set -eu
cd "$(dirname "$0")/.."
dest=$1
rev=$(awk '$1 == "rev" {print $2}' deploy/kistack.lock)
want=$(awk '$1 == "skills" {print $2}' deploy/kistack.lock)
case $rev in ''|*[!0-9a-f]*) echo "build-kistack: no rev in deploy/kistack.lock" >&2; exit 1;; esac
sha() { if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi; }
cache=${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/kistack-src
mkdir -p "$cache"
tgz=$cache/$rev.tar.gz
if [ ! -s "$tgz" ]; then
  curl -fsSL --max-time 300 -o "$tgz.new" "https://codeload.github.com/American-Embedded/KiStack/tar.gz/$rev"
  mv "$tgz.new" "$tgz"
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -xzf "$tgz" -C "$work" --strip-components=1
[ -d "$work/skills" ] || { echo "build-kistack: no skills/ at $rev" >&2; exit 1; }
if [ -n "$(find "$work/skills" -type l | head -1)" ]; then
  echo "build-kistack: symlinks in skills/ at $rev: refusing" >&2; exit 1
fi
got=$(cd "$work" && find skills -type f | LC_ALL=C sort | while IFS= read -r f; do printf '%s  %s\n' "$(sha < "$f" | cut -d' ' -f1)" "$f"; done | sha | cut -d' ' -f1)
if [ "$got" != "$want" ]; then
  echo "build-kistack: skills/ at $rev hashes to $got, not $want (deploy/kistack.lock)" >&2; exit 1
fi
rm -rf "$dest.new"
mkdir -p "$dest.new/.claude-plugin"
cp -R "$work/skills" "$dest.new/skills"
[ -f "$work/LICENSE" ] && cp "$work/LICENSE" "$dest.new/LICENSE"
printf '%s\n' '{"name":"kistack","description":"KiStack KiCad skills by American Embedded"}' > "$dest.new/.claude-plugin/plugin.json"
printf '%s\n' "$rev" > "$dest.new/REVISION"
rm -rf "$dest"
mv "$dest.new" "$dest"
echo "$dest: KiStack $rev ($(find "$dest/skills" -type f | wc -l | tr -d ' ') files)"
