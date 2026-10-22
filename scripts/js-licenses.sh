#!/bin/sh
# The notices a backplane-serve built by scripts/build-js.sh must ship with:
# Bun's (MIT, with JavaScriptCore/WebKit under LGPL-2 and tinycc under
# LGPL-2.1: the source and relinking offer is in deploy/licenses/bun/NOTICE.md)
# and Bend's (Apache-2.0: its runtime and Base are in the emitted JS). The
# texts are pinned by hash; Bun's is the version's own LICENSE.md, so this
# stops when the Bun in use is another version.
#   scripts/js-licenses.sh DIR       writes DIR/bun and DIR/bend
set -eu
cd "$(dirname "$0")/.."
out=${1:?js-licenses.sh DIR}
BUN_VERSION=1.4.3
BUN_LICENSE=056696884250b0d682365260cf1487a6501b1665a343ec60e23a1e647043c572
BEND_LICENSE=0beb288abd3d067e231f3fbe7df1f8ee37344061fc67f22018150a19e4b26c35
sum() { { sha256sum "$1" 2>/dev/null || shasum -a 256 "$1"; } | cut -c1-64; }
[ "$(sum deploy/licenses/bun/LICENSE.md)" = "$BUN_LICENSE" ] || { echo "js-licenses: deploy/licenses/bun/LICENSE.md is not Bun $BUN_VERSION's" >&2; exit 1; }
[ "$(sum deploy/licenses/bend/LICENSE)" = "$BEND_LICENSE" ] || { echo "js-licenses: deploy/licenses/bend/LICENSE changed" >&2; exit 1; }
bun=$(command -v bun || echo "$HOME/.bun/bin/bun")
have=$("$bun" --version 2>/dev/null || echo none)
# a release pins Bun (CI and release-bootstrap: 1.4.3); a dev build may run
# another, and is told its notices name 1.4.3
if [ "$have" != "$BUN_VERSION" ]; then
  [ "${BACKPLANE_LICENSES_STRICT:-}" = 1 ] && { echo "js-licenses: bun is $have, the notices are for $BUN_VERSION" >&2; exit 1; }
  echo "js-licenses: bun is $have; the notices are for $BUN_VERSION" >&2
fi
mkdir -p "$out/bun" "$out/bend"
cp deploy/licenses/bun/LICENSE.md deploy/licenses/bun/NOTICE.md "$out/bun/"
cp deploy/licenses/bend/LICENSE deploy/licenses/bend/NOTICE.md "$out/bend/"
