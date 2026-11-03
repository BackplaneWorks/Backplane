#!/bin/sh
# KiStack's skills as a build ships them, into DEST beside the hub: the copy
# tracked in this repository (deploy/kistack, pinned and hashed in
# deploy/kistack.lock), with a plugin.json of ours and a REVISION. Nothing
# is fetched; a copy that is missing or not the pinned commit stops the
# build (scripts/kistack.py).
#   scripts/build-kistack.sh DEST      (e.g. dist/kistack)
set -eu
cd "$(dirname "$0")/.."
exec python3 scripts/kistack.py build "${1:?build-kistack.sh DEST}"
