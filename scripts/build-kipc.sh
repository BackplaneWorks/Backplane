#!/bin/sh
# Build the KiCad IPC helper into one executable: dist/backplane-kipc.
# It needs libnng at run time (the KiCad runtime's own, else the system's);
# the API schema comes from the runtime's descriptor set.
set -e
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/tools/kipc"
bun install --frozen-lockfile 2>/dev/null || bun install
mkdir -p "$root/dist"
bun build.ts "$root/dist/backplane-kipc"
ls -la "$root/dist/backplane-kipc"
