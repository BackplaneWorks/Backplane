#!/bin/sh
# Build pinned local renderer/worker assets. These load only on a quality request.
set -eu
cd "$(dirname "$0")/.."
out=${1:-build/viewer/vendor}
(cd tools/viewer && bun install --frozen-lockfile --ignore-scripts)
bun tools/viewer/build.mjs "$out"
