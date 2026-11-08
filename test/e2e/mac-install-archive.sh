#!/bin/sh
# Installs a release's darwin-arm64 archive with its own install.sh into ./inst and prints the build folder (also appended to $GITHUB_ENV as BUILD).
#   GH_TOKEN=... test/e2e/mac-install-archive.sh DRY_RUN_ID TAG
# The dry run's release-dry-run artifact carries the real install.sh (filled in with each archive's sha256); when a dry run has no publish job only the archive is
# there, and the installer is made from deploy/install.sh.in the way release.yml does.
set -e
run=$1; tag=$2
if gh run download "$run" -n release-dry-run -D rel 2>/dev/null; then echo "installer: the dry run's own install.sh"
else
  gh run download "$run" -n archive-darwin-arm64 -D rel
  sha=$(shasum -a 256 "rel/backplane-$tag-darwin-arm64-native.tar.gz" | cut -d' ' -f1)
  sed -e "s|@REPO@|${GITHUB_REPOSITORY:-BackplaneWorks/Backplane}|" -e "s|@VERSION@|$tag|" -e "s|@SHA_LINUX_X64@||" -e "s|@SHA_LINUX_ARM64@||" -e "s|@SHA_DARWIN_ARM64@|$sha|" deploy/install.sh.in > rel/install.sh
  ! grep -q '@[A-Z_0-9]*@' rel/install.sh
  echo "installer: made from deploy/install.sh.in (no publish job in that run)"
fi
export BACKPLANE_ARCHIVE="$PWD/rel/backplane-$tag-darwin-arm64-native.tar.gz" BACKPLANE_PREFIX="$PWD/inst" BACKPLANE_HOME="$PWD/inst-home" BACKPLANE_NO_DESKTOP_APP=
sh rel/install.sh --quiet
B="$PWD/inst/builds/$tag"; ls -l "$B"; echo "commit: $(cat "$B/COMMIT")"
[ -z "${GITHUB_ENV:-}" ] || echo "BUILD=$B" >> "$GITHUB_ENV"
