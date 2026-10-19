#!/bin/sh
# The production CBOR codec used by synthetic hub clients in effect tests.
set -eu
cd "$(dirname "$0")/.."
. scripts/bendok.sh
bend_check test/wire/wire.bend
mkdir -p build
bend test/wire/index.html -o build/wire
