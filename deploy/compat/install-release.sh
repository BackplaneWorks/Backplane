#!/bin/sh
# 0.14 → 0.15, run by 0.14's own updater (src/server/update.bend, tag v0.14.1): it found the asset `backplane-<tag>-<platform>.tar.gz`
# (linux-x64: `-linux-x64-cef.tar.gz`), checked it against the release's SHA256SUMS, unpacked it, and runs this file from it:
#   sh <src>/install-release.sh <src> <prefix>      (prefix: 0.14's install folder, where its backplane-serve lives)
# then re-execs <prefix>/backplane-serve with 0.14's own arguments. Nothing here starts or stops anything, and 0.14's home is never touched.
#
# It lays 0.15 out as deploy/install.sh.in does (builds/<tag>/, the guard at <prefix>/backplane) and keeps 0.14 runnable beside it:
#   <prefix>/backplane-serve.0.14   0.14's server, copied before the shim replaces it: the build the guard goes back to if 0.15 fails
#   <prefix>/backplane.0.14         0.14's launcher
#   <prefix>/builds/<tag>/EXPECT    the version and commit the release's manifest names: the guard's identity check on probation
#   <prefix>/NEXT                   <tag>: what the shim asks the guard to move to
#   <prefix>/backplane-serve        backplane-serve.sh from this archive: on 0.14's re-exec it starts the guard (docs/deploy.md)
# Any failure leaves 0.14 as it was: everything is written beside and renamed into place last.
set -eu
src=$1 prefix=$2
tag=$(cat "$src/VERSION")
case $tag in v[0-9]*) ;; *) echo "release has no VERSION" >&2; exit 1 ;; esac
for f in native.tar.gz manifest.json backplane-serve.sh; do [ -f "$src/$f" ] || { echo "release missing $f" >&2; exit 1; }; done

# the native archive is the one the manifest names (0.14 checked this archive's sha256; this checks what is inside it against the manifest)
want=$(tr -d ' \n' < "$src/manifest.json" | sed -n "s/.*\"name\":\"$(cat "$src/NATIVE")\",\"sha256\":\"\([0-9a-f]*\)\".*/\1/p")
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$src/native.tar.gz" | cut -c1-64); else got=$(shasum -a 256 "$src/native.tar.gz" | cut -c1-64); fi
[ -n "$want" ] && [ "$want" = "$got" ] || { echo "native archive does not match the manifest; nothing replaced" >&2; exit 1; }
version=$(tr -d ' \n' < "$src/manifest.json" | sed -n 's/.*"version":"\([^"]*\)".*/\1/p')
commit=$(tr -d ' \n' < "$src/manifest.json" | sed -n 's/.*"commit":"\([0-9a-f]*\)".*/\1/p')
[ -n "$version" ] && [ -n "$commit" ] || { echo "manifest names no version or commit; nothing replaced" >&2; exit 1; }

mkdir -p "$prefix/builds"
if [ ! -d "$prefix/builds/$tag" ]; then
  stage="$prefix/builds/.stage-$tag-$$"; rm -rf "$stage"; mkdir "$stage"
  tar -xzf "$src/native.tar.gz" -C "$stage" --no-same-owner
  for f in backplane-hub backplane backplane-gpui; do [ -x "$stage/$f" ] || { rm -rf "$stage"; echo "release has no $f; nothing replaced" >&2; exit 1; }; done
  [ -z "$(find "$stage" -type l | head -1)" ] || { rm -rf "$stage"; echo "release holds a link; nothing replaced" >&2; exit 1; }
  printf '{"version":"%s","commit":"%s"}\n' "$version" "$commit" > "$stage/EXPECT"
  mv "$stage" "$prefix/builds/$tag"
fi

# 0.14 set aside, once: the shim's marker says the server here is already ours
if ! grep -q BACKPLANE-0.15-SHIM "$prefix/backplane-serve" 2>/dev/null; then
  cp -p "$prefix/backplane-serve" "$prefix/backplane-serve.0.14.new" && mv -f "$prefix/backplane-serve.0.14.new" "$prefix/backplane-serve.0.14"
  [ ! -f "$prefix/backplane" ] || grep -q 'backplane hub' "$prefix/backplane" 2>/dev/null || cp -p "$prefix/backplane" "$prefix/backplane.0.14"
fi
# the guard, then the shim, each beside and renamed over (0.14's running server keeps its own file open; its re-exec finds the shim)
cp "$prefix/builds/$tag/backplane" "$prefix/backplane.new" && chmod 755 "$prefix/backplane.new" && mv -f "$prefix/backplane.new" "$prefix/backplane"
printf '%s\n' "$tag" > "$prefix/NEXT.new" && mv -f "$prefix/NEXT.new" "$prefix/NEXT"
cp "$src/backplane-serve.sh" "$prefix/backplane-serve.new" && chmod 755 "$prefix/backplane-serve.new" && mv -f "$prefix/backplane-serve.new" "$prefix/backplane-serve"
echo "Backplane $tag is laid out in $prefix; 0.14 restarts into it"
