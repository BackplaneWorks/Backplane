#!/bin/sh
# Build the browser helper into one executable: dist/backplane-browser.
# It drives a Chromium found at run time (see tools/browser/README.md);
# no browser is bundled.
set -e
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/scripts/bun-pin.sh"; bun_pin_check bun
cd "$root/tools/browser"
bun install --frozen-lockfile 2>/dev/null || bun install
mkdir -p "$root/dist"
bun build.ts "$root/dist/backplane-browser"
ls -la "$root/dist/backplane-browser"
