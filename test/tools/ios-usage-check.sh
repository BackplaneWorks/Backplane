#!/bin/sh
set -eu
. scripts/bendok.sh
mkdir -p build/ios-reading-usage
bend_check src/mobile/app.bend > build/ios-reading-usage/preflight.log 2>&1 || { cat build/ios-reading-usage/preflight.log; exit 1; }
for t in test/ios_usage_test.bend test/hubs_test.bend test/mhalves_test.bend; do
  bend "$t" > "build/ios-reading-usage/$(basename "$t").log" 2>&1
  if grep -q '^FAIL\|^Error:' "build/ios-reading-usage/$(basename "$t").log"; then cat "build/ios-reading-usage/$(basename "$t").log"; exit 1; fi
  grep -q '^ok ' "build/ios-reading-usage/$(basename "$t").log"
  echo "$t PASS"
done
scripts/build-mobile.sh --js > build/ios-reading-usage/bridge-build.log 2>&1
sha256sum src/mobile/hubs.bend mobile/build/assets/bridge.js > build/ios-reading-usage/bridge-inputs.sha256
echo 'PASS usage/multihub/halves and production bridge'
